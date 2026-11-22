# Bidrag til RustDesk

RustDesk er åpene for bidrag fra alle. Her er reglene for de som har lyst til å
hjelpe oss:

## Bidrag

Bidrag til RustDesk eller deres avhengigheter burde være i form av GitHub pull requests.
Hver pull request vill bli sett igjennom av en kjerne bidrager (noen med autoritet til
å godkjenne endringene) og enten bli sendt til main treet eller respondert med
tilbakemelding på endringer som er nødvendig. Alle bidrag burde følge dette formate
også de fra kjerne bidragere.

Om du ønsker å jobbe på en issue må du huske å gjøre krav på den først. Dette
kann gjøres ved å kommentere på den GitHub issue-en du ønsker å jobbe på.
Dette er for å hindre duplikat innsats på samme problem.

## Pull Request Sjekkliste

- Lag en gren fra master grenen og, hvis det er nødvendig, rebase den til den nåværende
  master grenen før du sender inn din pull request. Hvis ikke dette gjøres på rent
  vis vill du bli spurt om å rebase dine endringer.

- Commits burde være så små som mulig, samtidig som de må være korrekt uavhenging av hverandre
  (hver commit burde kompilere og bestå tester).

- Commits burde være akkopaniert med en Developer Certificate of Origin
  (http://developercertificate.org), som indikerer att du (og din arbeidsgiver
  i det tilfellet) godkjenner å bli knyttet til vilkårene av [prosjekt lisensen](../LICENCE).
  Ved bruk av git er dette `-s` opsjonen til `git commit`.

- Hvis dine endringer ikke blir sett eller hvis du trenger en spesefik person til
  å se på dem kan du @-svare en med autoritet til å godkjenne dine endringer.
  Dette kann gjøres i en pull request, en kommentar eller via epost på [email](mailto:info@rustdesk.com).

- Legg til tester relevant til en fikset bug eller en ny tilgjengelighet.

For spesefike git instruksjoner, se [GitHub workflow 101](https://github.com/servo/servo/wiki/GitHub-workflow).

## Oppførsel

https://github.com/rustdesk/rustdesk/blob/master/docs/CODE_OF_CONDUCT.md

## Kommunikasjon

RustDesk bidragere burker [Discord](https://discord.gg/nDceKgxnkV).
