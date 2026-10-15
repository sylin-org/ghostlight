// The service publishes a hand edit to a policy file on its next one-second check. A journey that
// edits a policy while the service runs waits for that check before it relies on the edit.
export const POLICY_REFRESH_MS = 1500;

/** Resolve once the running service has had time to publish a policy file edit. */
export const policyRefreshed = () => new Promise((done) => setTimeout(done, POLICY_REFRESH_MS));
