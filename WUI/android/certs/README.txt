Certificates placed here (.crt, .pem or .cer) are built into DEBUG builds of the Healoo
Android app, so test devices trust the server without installing anything.

For the healoo-backend LAN trial server, copy its certificate authority here:
    docker compose cp care-api:/data/tls/healoo-local-ca.crt .      (in healoo-backend/deploy/lan)
    then copy healoo-local-ca.crt into this folder.
(In the Healoo repository the build also finds Server/deploy/lan/healoo-local-ca.crt
 automatically; with separate zips, healoo-backend/deploy/lan/healoo-local-ca.crt.)

Rebuild after changing files here:  .\gradlew.bat installDebug
The build output lists each bundled certificate.

If the server's certificate authority changes (for example after "docker compose down -v"),
copy the new file here and rebuild.

Release builds never include these certificates.
Never put private keys (.key files) in this folder.
