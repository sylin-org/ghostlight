# Windows fleet credential intake

Date: 2026-09-09. Machine: LEO-DESKTOP-02.

The GitHub connector authenticates as lbotinelly and reports admin access to
sylin-org/ghostlight. Local Git Credential Manager has no noninteractive stored
credential, so local authenticated Git push is not yet available.

Commit identity: Leo Botinelly <leonardo.botinelly@gmail.com>.
Local peer-fix commit: 5afcf7b9cc683a2eb88071c1b54d00e212420c85.
The local installer investigation remains uncommitted and is not included here.
This report was written through the authenticated GitHub connector.

Reuse the existing RSA-4096 intake key, named leo-desktop-02-rsa4096.pk8.
It is file-backed with access restricted to LEO-DESKTOP-02\\leo; it is not CNG.
Only its public SubjectPublicKeyInfo appears below. Envelope encryption is
RSA-OAEP SHA-256 with MGF1 SHA-256. No token or private key is in this report.

FLEET_AUTH_SHA256 c2854b77a971d1a997b4ab854bae8757452a5adf0070b1d129f10d4b51ca16e7

FLEET_AUTH_PUBLIC_KEY MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEArLY0c+zKv5zjQCBBgcv7lPr+RAJKe/3+T46DOkDyt7wZCfD0FGyKk4ccxwX0WPuLow7Pi6mSbWzezW+5nqi+MyVSboYmN5XFnqbxwTTm9ebKPp5e/tMCwWAfcu7to1GtHKXsKpUK0wOje3i0Mg//Ydzg2d6UH2VQkRW0HUASDaDhyd3zMNClbVbSvDSnLNCvOd2EqElupcTbpu60d/DxmDOVc+LJ6say/8v0C7AEnA/2TGaKvV8lWOgbBeHp6O9qfspWZfI6eV+s2rNVlrmdJLQzyzeq1YCWu/R0iO23RzYEa1Ocdh7M7JZgkdWy99rEU2iIvF5DcV35qIlBVEBdBQUN5nCIn9IRjRkYnFW3zLuQkdluDNXl9AQ+q5GynkqQccRbmPe+P5ogcLniwSbw2ltIm4u199xTQpBO8myFmclhmr/dIzp357pyQUAswbQ+ojpxoqQNCJewB61SVOVscN4fa5tWm/DdPzwmLzVrQipzr8V2VbcEuKKMBcQNkyOPOr6NreX18kyI/w2xB4ywMoOSB/avGt48EkAQ/0gzTphIL6GFqStoJY/xlWIqMYOW/Gy6EUxX+5lNLLAm+luzKiIXwMkNE7hoj5TKC7D4NOawNmcxebLW6546lwT3Bew4L2gPJ8taaLuYFsotvFJFGu1xZjcfWzRddtc8W1K9oh0CAwEAAQ==
