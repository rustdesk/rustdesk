# 为RustDesk做贡献

Rust欢迎每一位贡献者，如果您有意向为我们做出贡献，请遵循以下指南：

## 贡献方式

对 RustDesk 或其依赖项的贡献需要通过 GitHub 的 Pull Request (PR) 的形式提交。每个 PR 都会由核心贡献者（即有权限合并代码的人）进行审核，审核通过后代码会合并到主分支，或者您会收到需要修改的反馈。所有贡献者，包括核心贡献者，提交的代码都应遵循此流程。

如果您希望处理某个问题，请先在对应的 GitHub issue 下发表评论，声明您将处理该问题，以避免该问题被多位贡献者重复处理。

## PR 注意事项

- 从 master 分支创建一个新的分支，并在提交PR之前，如果需要，将您的分支 变基(rebase) 到最新的 master 分支。如果您的分支无法顺利合并到 master 分支，您可能会被要求更新您的代码。

- 每次提交的改动应该尽可能少，并且要保证每次提交的代码都是正确的（即每个 commit 都应能成功编译并通过测试）。

- 每个提交都应附有开发者证书签名(http://developercertificate.org), 表明您（以及您的雇主，若适用）同意遵守项目[许可证条款](../LICENCE)。在使用 git 提交代码时，可以通过在 `git commit` 时使用 `-s` 选项加入签名

- 如果您的 PR 未被及时审核，或需要指定的人员进行审核，您可以通过在 PR 或评论中 @ 提到相关审核者，以及发送[电子邮件](mailto:info@rustdesk.com)的方式请求审核。

- 请为修复的 bug 或新增的功能添加相应的测试用例。

有关具体的 git 使用说明，请参考[GitHub workflow 101](https://github.com/servo/servo/wiki/GitHub-workflow).

## 行为准则

请遵守项目的[贡献者公约行为准则](./CODE_OF_CONDUCT-ZH.md)。


## 沟通渠道

RustDesk 的贡献者主要通过 [Discord](https://discord.gg/nDceKgxnkV) 进行交流。
