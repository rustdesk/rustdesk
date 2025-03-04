#  RustDesk 기여 가이드라인

RustDesk는 모든 사람의 기여를 환영합니다. 만약 RustDesk에 기여하고 싶다면 아래 가이드를 참고해주세요:

## 기여 방식

RustDesk 또는 종속성에 대한 기여는 GitHub Pull Request 형태로 이루어져야 합니다.
모든 Pull Request는 코어 기여자가 검토하며, 메인 저장소에 반영되거나 필요한 수정 사항에 대한 피드백을 받습니다.
모든 기여는 이 형식을 따라야 합니다.

특정 이슈에 작업하고 싶다면, 먼저 GitHub 이슈에 댓글을 달아 작업하겠다고 알려주세요.
이는 동일한 작업에 대해 중복 기여가 발생하는 것을 방지하기 위함입니다.

## Pull Request Checklist

- master 브랜치에서 새 브랜치를 생성하고 작업하세요.<br/>
필요한 경우 PR 제출 전에 최신 master 브랜치에 리베이스(rebase)하세요.<br/>
충돌이 발생하면 기여자가 직접 해결해야 합니다.

- 커밋은 가능한 한 작고 독립적인 단위로 작성하세요.<br/>
각 커밋은 독립적으로 빌드와 테스트를 통과해야 합니다.

- 커밋에는 반드시 Developer Certificate of Origin (http://developercertificate.org) 서명이 포함되어야 합니다.<br/> 
이는 기여자(및 소속된 고용주가 있을 경우) 가 [프로젝트 라이선스](../LICENCE) 에 동의함을 나타냅니다.<br/>
Git에서는 `git commit` 명령어에 `-s` 옵션을 사용해 서명을 추가할 수 있습니다.

- PR이 검토되지 않거나 특정 리뷰어가 필요하면, 
<br/> PR이나 댓글에서 리뷰어를 태그하거나 [이메일](mailto:info@rustdesk.com)로 리뷰를 요청할 수 있습니다.

- 수정된 버그나 추가된 기능과 관련된 테스트 코드를 포함해주세요.

Git 사용에 대한 자세한 내용은 [GitHub workflow 101](https://github.com/servo/servo/wiki/GitHub-workflow)을 참조하세요.

## 행동 강령

https://github.com/rustdesk/rustdesk/blob/master/docs/CODE_OF_CONDUCT.md

## 커뮤니케이션

RustDesk 기여자들은 [Discord](https://discord.gg/nDceKgxnkV)에서 활동하고 있습니다.
