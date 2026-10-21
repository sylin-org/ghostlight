//! Customer-facing key, signing, and publication commands for managed policy.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context as _, Result};
use serde_json::Value;
use zeroize::Zeroizing;

use super::{bundle, crypto, encode_hex, manifest, validate_bootstrap, Bootstrap};
use crate::install::record::{atomic_replace_keeping_access, create_private};

const ED25519_SEED_FILE: &str = "policy-ed25519.seed";
const MLDSA_SEED_FILE: &str = "policy-mldsa65.seed";
type Flags<'a> = Vec<(&'a str, &'a str)>;
type ParsedArguments<'a> = (Vec<&'a str>, Flags<'a>);

/// One offline organization policy-authoring command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// Generate a new customer-owned signing keypair.
    Keygen {
        /// Directory that receives private seed files.
        directory: PathBuf,
        /// Generate only the required classical leg.
        ed25519_only: bool,
    },
    /// Print public bootstrap keys derived from private seed files.
    PublicKey {
        /// Ed25519 private seed file.
        ed25519_seed: PathBuf,
        /// Optional ML-DSA-65 private seed file.
        mldsa_seed: Option<PathBuf>,
    },
    /// Sign a schema-4 manifest at an explicit sequence.
    Sign(SignOptions),
    /// Sign at the next sequence and print a ready bootstrap.
    Publish(PublishOptions),
}

/// Shared signing inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignOptions {
    /// Schema-4 manifest path.
    pub manifest: PathBuf,
    /// Ed25519 private seed path.
    pub ed25519_seed: PathBuf,
    /// Optional ML-DSA-65 private seed path.
    pub mldsa_seed: Option<PathBuf>,
    /// Monotonic publish sequence.
    pub sequence: u64,
    /// Signed bundle destination.
    pub output: PathBuf,
    /// Optional signed additive organization presentation JSON.
    pub presentation: Option<PathBuf>,
}

/// Publication inputs, with sequence derived from the existing output bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishOptions {
    /// Shared signing inputs except sequence.
    pub manifest: PathBuf,
    /// Ed25519 private seed path.
    pub ed25519_seed: PathBuf,
    /// Optional ML-DSA-65 private seed path.
    pub mldsa_seed: Option<PathBuf>,
    /// Source fleet endpoints should fetch after deployment.
    pub source: String,
    /// Signed bundle destination.
    pub output: PathBuf,
    /// Optional signed additive organization presentation JSON.
    pub presentation: Option<PathBuf>,
}

/// Parse arguments after `ghostlight policy` for an authoring command.
pub fn parse(arguments: &[String]) -> Result<Command> {
    let Some(action) = arguments.first().map(String::as_str) else {
        bail!(usage());
    };
    match action {
        "keygen" => parse_keygen(&arguments[1..]),
        "pubkey" => parse_pubkey(&arguments[1..]),
        "sign" => parse_sign(&arguments[1..]).map(Command::Sign),
        "publish" => parse_publish(&arguments[1..]).map(Command::Publish),
        _ => bail!(usage()),
    }
}

/// Run one offline organization policy-authoring command.
pub fn run(command: &Command, out: &mut impl Write) -> Result<()> {
    match command {
        Command::Keygen {
            directory,
            ed25519_only,
        } => keygen(directory, *ed25519_only, out),
        Command::PublicKey {
            ed25519_seed,
            mldsa_seed,
        } => public_key(ed25519_seed, mldsa_seed.as_deref(), out),
        Command::Sign(options) => sign_explicit(options, out),
        Command::Publish(options) => publish(options, out),
    }
}

fn parse_keygen(arguments: &[String]) -> Result<Command> {
    match arguments {
        [directory] => Ok(Command::Keygen {
            directory: directory.into(),
            ed25519_only: false,
        }),
        [directory, option] if option == "--ed25519-only" => Ok(Command::Keygen {
            directory: directory.into(),
            ed25519_only: true,
        }),
        _ => bail!(usage()),
    }
}

fn parse_pubkey(arguments: &[String]) -> Result<Command> {
    let (positional, flags) = split_flags(arguments)?;
    let [ed25519_seed] = positional.as_slice() else {
        bail!(usage());
    };
    Ok(Command::PublicKey {
        ed25519_seed: ed25519_seed.into(),
        mldsa_seed: flag(&flags, "--mldsa-seed").map(PathBuf::from),
    })
}

fn parse_sign(arguments: &[String]) -> Result<SignOptions> {
    let (positional, flags) = split_flags(arguments)?;
    let [manifest] = positional.as_slice() else {
        bail!(usage());
    };
    Ok(SignOptions {
        manifest: manifest.into(),
        ed25519_seed: required_flag(&flags, "--ed25519-seed")?.into(),
        mldsa_seed: flag(&flags, "--mldsa-seed").map(PathBuf::from),
        sequence: required_flag(&flags, "--sequence")?
            .parse()
            .context("--sequence must be an unsigned integer")?,
        output: flag(&flags, "--out")
            .map_or_else(|| PathBuf::from("policy.bundle.json"), PathBuf::from),
        presentation: flag(&flags, "--presentation").map(PathBuf::from),
    })
}

fn parse_publish(arguments: &[String]) -> Result<PublishOptions> {
    let (positional, flags) = split_flags(arguments)?;
    let [manifest] = positional.as_slice() else {
        bail!(usage());
    };
    Ok(PublishOptions {
        manifest: manifest.into(),
        ed25519_seed: required_flag(&flags, "--ed25519-seed")?.into(),
        mldsa_seed: flag(&flags, "--mldsa-seed").map(PathBuf::from),
        source: required_flag(&flags, "--source")?.into(),
        output: flag(&flags, "--out")
            .map_or_else(|| PathBuf::from("policy.bundle.json"), PathBuf::from),
        presentation: flag(&flags, "--presentation").map(PathBuf::from),
    })
}

fn split_flags(arguments: &[String]) -> Result<ParsedArguments<'_>> {
    let mut positional = Vec::new();
    let mut flags = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        if let Some((name, value)) = argument.split_once('=') {
            if !name.starts_with("--") || value.is_empty() {
                bail!("invalid policy option {argument}");
            }
            flags.push((name, value));
        } else if argument.starts_with("--") {
            let value = arguments
                .get(index + 1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| anyhow!("{argument} needs a value"))?;
            flags.push((argument, value));
            index += 1;
        } else {
            positional.push(argument);
        }
        index += 1;
    }
    for (index, (name, _)) in flags.iter().enumerate() {
        if flags[..index].iter().any(|(earlier, _)| earlier == name) {
            bail!("duplicate policy option {name}");
        }
        if !matches!(
            *name,
            "--ed25519-seed"
                | "--mldsa-seed"
                | "--sequence"
                | "--source"
                | "--out"
                | "--presentation"
        ) {
            bail!("unknown policy option {name}");
        }
    }
    Ok((positional, flags))
}

fn flag<'a>(flags: &'a [(&str, &str)], name: &str) -> Option<&'a str> {
    flags
        .iter()
        .find_map(|(candidate, value)| (*candidate == name).then_some(*value))
}

fn required_flag<'a>(flags: &'a [(&str, &str)], name: &str) -> Result<&'a str> {
    flag(flags, name).ok_or_else(|| anyhow!("{name} is required"))
}

fn keygen(directory: &Path, ed25519_only: bool, out: &mut impl Write) -> Result<()> {
    fs::create_dir_all(directory)
        .with_context(|| format!("create key directory {}", directory.display()))?;
    let ed25519_path = directory.join(ED25519_SEED_FILE);
    let mldsa_path = directory.join(MLDSA_SEED_FILE);
    if ed25519_path.exists() || (!ed25519_only && mldsa_path.exists()) {
        bail!("policy key generation never overwrites an existing seed file");
    }
    let mut ed25519 = Zeroizing::new([0_u8; 32]);
    getrandom::getrandom(&mut *ed25519)
        .map_err(|error| anyhow!("generate Ed25519 seed: {error}"))?;
    write_seed(&ed25519_path, &*ed25519)?;
    writeln!(out, "Created {}", ed25519_path.display())?;
    if !ed25519_only {
        let mut mldsa = Zeroizing::new([0_u8; 32]);
        getrandom::getrandom(&mut *mldsa)
            .map_err(|error| anyhow!("generate ML-DSA-65 seed: {error}"))?;
        write_seed(&mldsa_path, &*mldsa)?;
        writeln!(out, "Created {}", mldsa_path.display())?;
    }
    writeln!(out, "Keep these private seed files offline. Use 'ghostlight policy pubkey' to print the public bootstrap keys.")?;
    Ok(())
}

fn public_key(ed25519_path: &Path, mldsa_path: Option<&Path>, out: &mut impl Write) -> Result<()> {
    let ed25519 = read_seed(ed25519_path)?;
    writeln!(
        out,
        "pubkey_ed25519 {}",
        encode_hex(&crypto::signing::ed25519_public(&ed25519))
    )?;
    if let Some(path) = mldsa_path {
        let mldsa = read_seed(path)?;
        writeln!(
            out,
            "pubkey_mldsa {}",
            encode_hex(&crypto::signing::mldsa_public(&mldsa))
        )?;
    }
    Ok(())
}

fn sign_explicit(options: &SignOptions, out: &mut impl Write) -> Result<()> {
    if options.sequence == 0 {
        bail!("managed policy sequence must be at least 1");
    }
    let bytes = signed_bytes(
        &options.manifest,
        &options.ed25519_seed,
        options.mldsa_seed.as_deref(),
        options.sequence,
        options.presentation.as_deref(),
    )?;
    write_bundle(&options.output, &bytes)?;
    writeln!(
        out,
        "Signed policy sequence {} to {}.",
        options.sequence,
        options.output.display()
    )?;
    write!(out, "{}", bundle::armor(&bytes))?;
    Ok(())
}

fn publish(options: &PublishOptions, out: &mut impl Write) -> Result<()> {
    let ed25519 = read_seed(&options.ed25519_seed)?;
    let mldsa = options.mldsa_seed.as_deref().map(read_seed).transpose()?;
    let key = crypto::verification_key(
        &crypto::signing::ed25519_public(&ed25519),
        mldsa
            .as_ref()
            .map(|seed| crypto::signing::mldsa_public(seed))
            .as_ref(),
    )
    .expect("derived signing keys are valid");
    // The bootstrap is checked before anything is signed or written, so a refused source leaves
    // the published bundle as it was (G-D2).
    let bootstrap = Bootstrap {
        source: options.source.clone(),
        pubkey_ed25519: encode_hex(&crypto::signing::ed25519_public(&ed25519)),
        pubkey_mldsa: mldsa
            .as_ref()
            .map(|seed| encode_hex(&crypto::signing::mldsa_public(seed))),
        ..Bootstrap::default()
    };
    validate_bootstrap(&bootstrap).map_err(anyhow::Error::msg)?;
    let sequence = match fs::read(&options.output) {
        Ok(existing) => bundle::verify(&existing, &key)
            .context("existing output is not a bundle signed by these keys")?
            .sequence
            .checked_add(1)
            .ok_or_else(|| anyhow!("managed policy sequence is exhausted"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 1,
        Err(error) => {
            return Err(error).with_context(|| format!("read {}", options.output.display()))
        }
    };
    let bytes = signed_bytes_from_seeds(
        &options.manifest,
        &ed25519,
        mldsa.as_deref(),
        sequence,
        options.presentation.as_deref(),
    )?;
    write_bundle(&options.output, &bytes)?;
    writeln!(
        out,
        "Published policy sequence {sequence} to {}.",
        options.output.display()
    )?;
    writeln!(
        out,
        "Provision this managed.json through administrator tooling:"
    )?;
    writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "source": bootstrap.source,
            "pubkey_ed25519": bootstrap.pubkey_ed25519,
            "pubkey_mldsa": bootstrap.pubkey_mldsa,
        }))?
    )?;
    Ok(())
}

fn signed_bytes(
    manifest_path: &Path,
    ed25519_path: &Path,
    mldsa_path: Option<&Path>,
    sequence: u64,
    presentation_path: Option<&Path>,
) -> Result<Vec<u8>> {
    let ed25519 = read_seed(ed25519_path)?;
    let mldsa = mldsa_path.map(read_seed).transpose()?;
    signed_bytes_from_seeds(
        manifest_path,
        &ed25519,
        mldsa.as_deref(),
        sequence,
        presentation_path,
    )
}

fn signed_bytes_from_seeds(
    manifest_path: &Path,
    ed25519: &[u8; 32],
    mldsa: Option<&[u8; 32]>,
    sequence: u64,
    presentation_path: Option<&Path>,
) -> Result<Vec<u8>> {
    let manifest_text = fs::read_to_string(manifest_path)
        .with_context(|| format!("read policy {}", manifest_path.display()))?;
    manifest::parse(&manifest_text, &manifest_path.display().to_string())
        .map_err(anyhow::Error::new)?;
    let manifest: Value = serde_json::from_str(&manifest_text)?;
    let presentation = presentation_path
        .map(|path| {
            let bytes =
                fs::read(path).with_context(|| format!("read presentation {}", path.display()))?;
            serde_json::from_slice::<bundle::Presentation>(&bytes)
                .with_context(|| format!("decode presentation {}", path.display()))
        })
        .transpose()?;
    bundle::validate_presentation(presentation.as_ref()).map_err(anyhow::Error::new)?;
    Ok(bundle::sign(
        ed25519,
        mldsa,
        sequence,
        manifest,
        presentation,
    ))
}

fn read_seed(path: &Path) -> Result<Zeroizing<[u8; 32]>> {
    let bytes =
        Zeroizing::new(fs::read(path).with_context(|| format!("read seed {}", path.display()))?);
    let seed = <[u8; 32]>::try_from(bytes.as_slice())
        .map_err(|_| anyhow!("seed {} must be exactly 32 bytes", path.display()))?;
    Ok(Zeroizing::new(seed))
}

/// Write a new seed file that only the current user can read. An existing file is refused.
fn write_seed(path: &Path, seed: &[u8]) -> Result<()> {
    let mut file = create_private(path)
        .with_context(|| format!("create private seed {} without overwriting", path.display()))?;
    file.write_all(seed)?;
    file.sync_all()?;
    Ok(())
}

/// Publish a signed bundle at `path`. A bundle is meant for other accounts and machines, so it
/// keeps the access a file there had, and a new one gets its directory's default access.
fn write_bundle(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_replace_keeping_access(path, bytes)
        .with_context(|| format!("replace {}", path.display()))
}

fn usage() -> &'static str {
    "usage: ghostlight policy keygen <directory> [--ed25519-only]\n       ghostlight policy pubkey <ed25519-seed> [--mldsa-seed <file>]\n       ghostlight policy sign <policy.json> --ed25519-seed <file> [--mldsa-seed <file>] --sequence <n> [--out <bundle>] [--presentation <json>]\n       ghostlight policy publish <policy.json> --ed25519-seed <file> [--mldsa-seed <file>] --source <path-or-https> [--out <bundle>] [--presentation <json>]"
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    use super::{parse, run, Command, ED25519_SEED_FILE, MLDSA_SEED_FILE};
    use crate::governance::managed::{ManagedAuthority, ManagedPaths};
    use crate::install::record::assert_owner_only;
    use crate::scratch::Scratch;

    /// Parse `arguments` after `ghostlight policy` and run them, returning what they printed.
    fn policy(arguments: &[String]) -> anyhow::Result<String> {
        let mut out = Vec::new();
        run(&parse(arguments)?, &mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    /// The value printed after `label` on its own line.
    fn printed<'a>(out: &'a str, label: &str) -> &'a str {
        out.lines()
            .find_map(|line| line.strip_prefix(label)?.strip_prefix(' '))
            .unwrap_or_else(|| panic!("{label} is not printed: {out}"))
    }

    /// Move `path`'s modified time past the last read, so the next refresh reads it.
    fn touched(path: &Path) {
        let file = fs::File::options().write(true).open(path).unwrap();
        let modified = file.metadata().unwrap().modified().unwrap();
        file.set_modified(modified + Duration::from_secs(2))
            .unwrap();
    }

    /// The governance guide (Publish signed managed policy), ADR-0055 Decision 3, ADR-0121
    /// Decision 4 and be5c2cf4: the authoring commands and an endpoint agree. Keys from keygen,
    /// the managed.json publish prints and the bundle it writes, into a directory that did not
    /// exist, verify under the managed authority at the next sequence each time; pubkey prints the
    /// same keys; and the armored bundle sign prints is accepted as it stands.
    #[test]
    fn what_the_authoring_commands_publish_an_endpoint_accepts() {
        const NAME: &str = "org";
        const SOURCE: &str = "policy.bundle";
        const PROVISION: &str = "Provision this managed.json";
        const ARMOR_BEGIN: &str = "-----BEGIN";
        const SIGNED_SEQUENCE: u64 = 3;
        let root = Scratch::named("policy-authoring");
        let keys = root.join("keys");
        let published = root.join("published");
        let manifest = root.join("policy.json");
        fs::write(
            &manifest,
            format!(r#"{{"schema":4,"name":"{NAME}","version":"1","grants":[]}}"#),
        )
        .unwrap();
        let ed25519 = keys.join(ED25519_SEED_FILE).display().to_string();
        let mldsa = keys.join(MLDSA_SEED_FILE).display().to_string();
        let bundle = published.join(SOURCE);
        let signing = |command: &str| {
            vec![
                command.to_owned(),
                manifest.display().to_string(),
                format!("--ed25519-seed={ed25519}"),
                format!("--mldsa-seed={mldsa}"),
            ]
        };

        policy(&["keygen".into(), keys.display().to_string()]).unwrap();
        let public = policy(&[
            "pubkey".into(),
            ed25519.clone(),
            "--mldsa-seed".into(),
            mldsa.clone(),
        ])
        .unwrap();
        let mut publish = signing("publish");
        publish.push(format!("--source={SOURCE}"));
        publish.push(format!("--out={}", bundle.display()));
        policy(&publish).unwrap();

        // G-D2: a source the bootstrap would refuse is refused before anything is written, so the
        // published bundle stays as it was.
        let first = fs::read(&bundle).unwrap();
        let mut insecure = signing("publish");
        insecure.push("--source=http://insecure.example/policy".into());
        insecure.push(format!("--out={}", bundle.display()));
        assert!(policy(&insecure).is_err());
        assert_eq!(fs::read(&bundle).unwrap(), first);

        let out = policy(&publish).unwrap();

        let (_, provisioned) = out.split_once(PROVISION).expect(PROVISION);
        let (_, descriptor) = provisioned.split_once('\n').unwrap();
        let descriptor: serde_json::Value = serde_json::from_str(descriptor).unwrap();
        for key in ["pubkey_ed25519", "pubkey_mldsa"] {
            assert_eq!(descriptor[key], printed(&public, key), "{key}");
        }
        let paths = ManagedPaths::under(&published);
        fs::write(&paths.bootstrap, descriptor.to_string()).unwrap();
        let mut authority = ManagedAuthority::new(paths);
        authority.refresh();
        assert!(authority.valid());
        assert_eq!(
            authority.sequence(),
            Some(2),
            "a second publish is sequence 2"
        );
        assert_eq!(authority.manifest().unwrap().name, NAME);

        let mut sign = signing("sign");
        sign.push(format!("--sequence={SIGNED_SEQUENCE}"));
        sign.push(format!("--out={}", root.join("other.bundle").display()));
        let out = policy(&sign).unwrap();
        let armored = &out[out
            .find(ARMOR_BEGIN)
            .expect("sign prints the armored bundle")..];
        fs::write(&bundle, armored).unwrap();
        touched(&bundle);
        authority.refresh();
        assert_eq!(authority.sequence(), Some(SIGNED_SEQUENCE));
    }

    /// e2b3ade3 and ADR-0203 Decision 11.1: keygen writes each private seed readable only by the
    /// user (a protected DACL on Windows, 0600 on Unix) and never overwrites an existing seed.
    #[test]
    fn keygen_seeds_are_readable_only_by_the_user() {
        let root = Scratch::named("policy-keygen");
        let keys = root.join("keys");
        let keygen = Command::Keygen {
            directory: keys.clone(),
            ed25519_only: false,
        };
        run(&keygen, &mut Vec::new()).unwrap();
        let seeds = [ED25519_SEED_FILE, MLDSA_SEED_FILE].map(|name| keys.join(name));
        let written = seeds.clone().map(|seed| fs::read(seed).unwrap());
        for (seed, bytes) in seeds.iter().zip(&written) {
            assert_eq!(bytes.len(), 32, "{}", seed.display());
            assert_owner_only(seed);
        }
        assert!(run(&keygen, &mut Vec::new()).is_err());
        assert_eq!(seeds.map(|seed| fs::read(seed).unwrap()), written);
    }
}
