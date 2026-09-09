
# Contribuer à RustDesk

RustDesk accueille les contributions de tous. Voici les directives si vous
envisagez de nous aider :

## Contributions

Les contributions à RustDesk ou à ses dépendances doivent être soumises sous
forme de pull requests GitHub. Chaque pull request sera examinée par un
contributeur principal (une personne ayant la permission d'intégrer des
correctifs) et sera soit intégrée dans la branche principale, soit accompagnée
de retours sur les modifications requises. Toutes les contributions doivent
suivre ce format, même celles des contributeurs principaux.

Si vous souhaitez travailler sur une issue, veuillez d'abord la revendiquer en
commentant sur l'issue GitHub indiquant que vous souhaitez la traiter. Cela
permet d'éviter les efforts en double de la part des contributeurs sur la même
issue.

## Liste de vérification pour les pull requests

- Partez de la branche master et, si nécessaire, effectuez un rebase sur la
  branche master actuelle avant de soumettre votre pull request. Si elle ne
  fusionne pas proprement avec master, il vous sera peut-être demandé de
  rebaser vos modifications.

- Les commits doivent être aussi petits que possible, tout en s'assurant que
  chaque commit est correct de manière indépendante (c.-à-d. que chaque commit
  doit compiler et passer les tests).

- Les commits doivent être accompagnés d'une signature Developer Certificate of
  Origin (http://developercertificate.org), indiquant que vous (et votre
  employeur le cas échéant) acceptez d'être liés par les termes de la
  [licence du projet](../LICENCE). Dans git, il s'agit de l'option `-s` de
  `git commit`.

- Si votre correctif n'est pas examiné ou si vous avez besoin qu'une personne
  spécifique l'examine, vous pouvez @-mentionner un relecteur pour demander une
  revue dans la pull request ou un commentaire, ou vous pouvez demander une
  revue par [e-mail](mailto:info@rustdesk.com).

- Ajoutez des tests relatifs au bug corrigé ou à la nouvelle fonctionnalité.

Pour des instructions git spécifiques, consultez le
[GitHub workflow 101](https://github.com/servo/servo/wiki/GitHub-workflow).

## Conduite

https://github.com/rustdesk/rustdesk/blob/master/docs/CODE_OF_CONDUCT.md

## Communication

Les contributeurs de RustDesk se retrouvent fréquemment sur
[Discord](https://discord.gg/nDceKgxnkV).
