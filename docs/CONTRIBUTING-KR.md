# RustDesk에 기여하기

RustDesk는 모든 분들의 기여를 환영합니다. RustDesk에 기여하고 싶으시다면 아래 가이드를 참고해 주세요:

## 기여 방법

RustDesk 프로젝트 또는 관련 라이브러리에 대한 기여는 GitHub 풀 리퀘스트(Pull Request) 형태로 이루어져야 합니다.
각 풀 리퀘스트는 핵심 기여자(패치 적용 권한이 있는 사람)가 검토하며,
메인 브랜치에 통합되거나 필요한 변경 사항에 대한 피드백을 받게 됩니다.
핵심 기여자를 포함한 모든 기여자는 이 형식을 따라야 합니다.

특정 이슈에 대해 작업하고 싶다면, 먼저 해당 GitHub 이슈에 댓글을 달아 작업 의사를 알려주세요.
이는 여러 기여자가 동일한 이슈에 대해 중복으로 작업하는 것을 방지하기 위함입니다.

## 풀 리퀘스트 체크리스트

- master 브랜치에서 새 브랜치를 만들고, 필요한 경우 Pull Request를 제출하기 전에 현재 master
  브랜치로 리베이스하세요. master 브랜치와 깔끔하게 병합(merge)되지 않으면 변경 사항을
  리베이스하도록 요청받을 수 있습니다.

- 커밋(commit)은 가능한 한 작게 유지하고, 각 커밋이 독립적으로 올바른지 (즉, 각 커밋이 컴파일되고 테스트를 통과하는지) 확인해야 합니다.

- 커밋에는 개발자 원본 증명서(DCO, Developer Certificate of Origin - http://developercertificate.org) 서명이 포함되어야 합니다. 이는 기여자(해당하는 경우
  기여자의 고용주 포함)가 [프로젝트 라이선스](../LICENCE) 조건에 동의함을 의미합니다.
  Git에서는 `git commit` 명령어에 `-s` 옵션을 사용합니다.

- 패치가 검토되지 않거나 특정 리뷰어의 검토가 필요하다면, 풀 리퀘스트나 댓글에서
  @멘션으로 리뷰어에게 알리거나 [이메일](mailto:info@rustdesk.com)로 검토를 요청할 수 있습니다.

- 수정한 버그나 추가한 기능과 관련된 테스트 코드를 포함해 주세요.

Git 사용에 대한 자세한 내용은 [GitHub workflow 101](https://github.com/servo/servo/wiki/GitHub-workflow) 문서를 참고하세요.

## 기여자 행동 강령

https://github.com/rustdesk/rustdesk/blob/master/docs/CODE_OF_CONDUCT.md

## 소통 채널

RustDesk 기여자들은 주로 [Discord](https://discord.gg/nDceKgxnkV)에서 소통합니다.
