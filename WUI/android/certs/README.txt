Certificates placed here (.crt, .pem or .cer) are built into DEBUG builds of the Healoo
Android app, so test devices trust the server without installing anything.

For the healoo-backend LAN trial server, copy its certificate authority here:
    docker compose cp care-api:/data/tls/healoo-local-ca.crt .      (in healoo-backend/deploy/lan)
    then copy healoo-local-ca.crt into this folder.
(In the Healoo repository the build also finds Server/deploy/lan/healoo-local-ca.crt
 automatically; with separate zips, healoo-backend/deploy/lan/healoo-local-ca.crt.)

For the internet profile run locally (Caddy with its own certificate authority), copy Caddy's
root certificate here (in Server/deploy/internet, with the caddy container running):
    docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt ../../../WUI/android/certs/healoo-internet-local-ca.crt
Copy root.crt only, not intermediate.crt (Caddy replaces that every 7 days).

Rebuild after changing files here:  .\gradlew.bat installDebug
The build output lists each bundled certificate.

If the server's certificate authority changes (for example after "docker compose down -v"),
copy the new file here and rebuild.

Release builds never include these certificates.
Never put private keys (.key files) in this folder.
