# Administrators file

`admins.enc` in this folder lists Healoo's platform administrators (and, optionally, hospital
administrators) with their names, emails and Healoo IDs. It is encrypted with a key phrase and is
never committed (see `.gitignore`). `care-api` reads it at every start and makes the accounts
match it: listed platform administrators get the role, anyone else loses it.

The key phrase is `ADMINS_KEY` in `deploy/internet/.env` (or `deploy/lan/.env`). Commands, run
from the deploy folder:

    docker compose run --rm care-api care-admin config init      # new file
    docker compose run --rm care-api care-admin config show
    docker compose run --rm care-api care-admin config add-platform "Asha Verma" asha@example.com -
    docker compose run --rm care-api care-admin config add-hospital "Ravi K" ravi@example.com - HL-1H0A1
    docker compose run --rm care-api care-admin config remove asha@example.com
    docker compose run --rm care-api care-admin config apply     # now, without a restart

Change the key phrase: run `care-admin config rekey` with `ADMINS_NEW_KEY` set
(`docker compose run --rm -e ADMINS_NEW_KEY=... care-api care-admin config rekey`), then put the
new phrase in `ADMINS_KEY`.

Losing the key phrase loses the file (it can't be decrypted); create a new one with `init`.
See Documentation/Administration_Design.md section 10.
