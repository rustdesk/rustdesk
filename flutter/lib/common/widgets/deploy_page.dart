import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:flutter_hbb/common.dart';
import 'dart:async';
import 'dart:math';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:flutter_hbb/models/user_model.dart';
import 'package:flutter_hbb/models/deploy_model.dart';
import 'package:window_manager/window_manager.dart';

class DeployPage extends StatefulWidget {
  final bool isDialogMode;
  final Function()? onClose;

  const DeployPage({
    Key? key,
    this.isDialogMode = false,
    this.onClose,
  }) : super(key: key);

  static Future<void> showAsDialog(BuildContext context) {
    gFFI.deployModel.error.value = '';
    return gFFI.dialogManager.show(
      (setState, close, context) => CustomAlertDialog(
        content: DeployPage(
          isDialogMode: true,
          onClose: close,
        ),
        actions: [
          dialogButton(
            'Close',
            onPressed: close,
            isOutline: true,
          ),
        ],
        onCancel: close,
      ),
    );
  }

  @override
  State<DeployPage> createState() => _DeployPageState();
}

class _DeployPageState extends State<DeployPage> {
  final TextEditingController _controller = TextEditingController();
  final RxString _errorTextEdit = ''.obs;
  final RxBool _isDeployCodeMode = false.obs;
  final TextEditingController _emailController = TextEditingController();
  final TextEditingController _passwordController = TextEditingController();
  final RxString _emailError = RxString('');
  final RxString _passwordError = RxString('');
  final RxBool _isLoading = false.obs;

  final Rx<DeployWithCodeResponse?> _deployResponse =
      Rx<DeployWithCodeResponse?>(null);
  final RxBool _showConfirmation = false.obs;
  final RxString _currentCode = ''.obs;
  final RxBool _isConfirmationMode = false.obs;

  final GlobalKey _contentKey = GlobalKey();

  @override
  void initState() {
    super.initState();
    _emailController.text = UserModel.getLocalUserInfo()?['email'] ?? '';
    _isDeployCodeMode.value = false;

    if (!widget.isDialogMode) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _checkContentSizeAndResizeWindow();
      });
    }
  }

  void _checkContentSizeAndResizeWindow() {
    final RenderBox? renderBox =
        _contentKey.currentContext?.findRenderObject() as RenderBox?;
    if (renderBox == null) return;

    final Size contentSize = renderBox.size;
    final Size screenSize = MediaQuery.of(context).size;

    if (contentSize.height > screenSize.height * 0.8) {
      windowManager.setSize(Size(max(350, contentSize.width + 50),
          min(700, contentSize.height + 100)));
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    _emailController.dispose();
    _passwordController.dispose();
    if (!widget.isDialogMode && bind.isIncomingOnly()) {
      windowManager.setSize(getIncomingOnlyHomeSize());
    }
    super.dispose();
  }

  Future<void> _deployWithCodeRequest(String code) async {
    if (code.length != 12) {
      _errorTextEdit.value = translate('deploy-code-requirement-tip');
      return;
    }
    if (!RegExp(r'^[A-Z0-9]{12}$').hasMatch(code)) {
      _errorTextEdit.value = translate('Invalid deploy code');
      return;
    }

    _isLoading.value = true;
    final resp = await gFFI.deployModel.deployWithCodeRequest(code);
    _isLoading.value = false;

    if (resp != null) {
      _deployResponse.value = resp;
      _showConfirmation.value = true;
      _currentCode.value = code;
    }
  }

  void _cancelDeployment() {
    _deployResponse.value = null;
    _showConfirmation.value = false;
    _currentCode.value = '';
  }

  void _checkCloseDialog() {
    if (widget.isDialogMode && gFFI.deployModel.isDeployed.value) {
      widget.onClose?.call();
    }
  }

  Future<void> _confirmDeployment() async {
    _isLoading.value = true;
    await gFFI.deployModel.deployWithCode(_currentCode.value);
    _isLoading.value = false;

    _showConfirmation.value = false;
    _deployResponse.value = null;
    _currentCode.value = '';

    if (gFFI.deployModel.error.isEmpty) {
      await gFFI.deployModel.checkDeploy();
    }
    _checkCloseDialog();
  }

  Future<void> _deployWithAccount() async {
    _emailError.value = '';
    _passwordError.value = '';
    _isLoading.value = true;

    try {
      if (_emailController.text.isEmpty) {
        _emailError.value = translate('Email missed');
        return;
      }
      if (_passwordController.text.isEmpty) {
        _passwordError.value = translate('Password missed');
        return;
      }
      await gFFI.deployModel.deployWithAccount(
          _emailController.text.trim(), _passwordController.text);
      if (gFFI.deployModel.error.isEmpty) {
        await gFFI.deployModel.checkDeploy();
      }
    } catch (e) {
      _passwordError.value = translate('Login failed');
    } finally {
      _isLoading.value = false;
      _checkCloseDialog();
    }
  }

  Future<void> _deployToCurrentAccount() async {
    _isLoading.value = true;
    try {
      await gFFI.deployModel.deployToLoginUser();
      if (gFFI.deployModel.error.isEmpty) {
        await gFFI.deployModel.checkDeploy();
      }
    } finally {
      _isLoading.value = false;
      _checkCloseDialog();
    }
  }

  void _handleBack() {
    if (_isConfirmationMode.value) {
      _cancelDeployment();
    } else if (_isDeployCodeMode.value) {
      _isDeployCodeMode.value = false;
      _controller.clear();
      _errorTextEdit.value = '';
    } else {
      if (widget.isDialogMode) {
        widget.onClose?.call();
      } else {
        gFFI.deployModel.showDeployPage.value = false;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return _buildHost(context);
  }

  Widget _buildHost(BuildContext context) {
    final model = gFFI.deployModel;
    return Obx(() {
      final bool isLoading =
          model.checking.value || model.deploying.value || _isLoading.value;

      _isConfirmationMode.value = _isDeployCodeMode.value &&
          _showConfirmation.value &&
          _deployResponse.value != null;

      body() {
        return Center(
          child: SingleChildScrollView(
            child: Container(
              key: _contentKey,
              padding: const EdgeInsets.all(24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (widget.isDialogMode &&
                      !_isConfirmationMode.value &&
                      _isDeployCodeMode.value)
                    IconButton(
                      icon: const Icon(Icons.arrow_back, color: Colors.grey),
                      onPressed: _handleBack,
                      tooltip: translate('Back'),
                    ),
                  _isConfirmationMode.value
                      ? _buildDeployConfirmation(context, isLoading)
                      : Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.center,
                          children: [
                            if (isLoading)
                              _buildLoadingState(model)
                            else
                              _buildHeader(context),
                            const SizedBox(height: 24),
                            if (_isDeployCodeMode.value)
                              _buildDeployCodeMode(context, isLoading)
                            else if (bind.isStandard() &&
                                gFFI.userModel.isLogin)
                              _buildLoggedInMode(context, isLoading)
                            else
                              _buildAccountMode(context, isLoading),
                            const SizedBox(height: 16),
                            if (gFFI.deployModel.error.isNotEmpty)
                              _buildErrorText(context),
                          ],
                        ),
                ],
              ),
            ),
          ),
        );
      }

      if (widget.isDialogMode) return body();

      return Scaffold(
        backgroundColor: Theme.of(context).colorScheme.background,
        appBar: AppBar(
          elevation: 0,
          automaticallyImplyLeading: false,
          backgroundColor: Colors.transparent,
          leading: IconButton(
            icon: const Icon(Icons.arrow_back, color: Colors.grey),
            onPressed: _handleBack,
            tooltip: translate('Back'),
          ),
        ),
        body: body(),
      );
    });
  }

  Widget _buildLoadingState(DeployModel model) {
    return Column(
      children: [
        const CircularProgressIndicator(),
        const SizedBox(height: 24),
        Text(
          model.checking.value
              ? translate('Checking deployment...')
              : model.deploying.value
                  ? translate('Deploying...')
                  : translate('Logging in...'),
          style: const TextStyle(
            fontSize: 20,
            fontWeight: FontWeight.bold,
          ),
        ),
      ],
    );
  }

  Widget _buildHeader(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Text(
          _isDeployCodeMode.value
              ? translate('Deploy with deploy code')
              : translate('Deploy with account'),
          style: const TextStyle(
            fontSize: 20,
            fontWeight: FontWeight.bold,
          ),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 12),
        _buildHeaderContent(context),
      ],
    );
  }

  Widget _buildHeaderContent(BuildContext context) {
    if (_isDeployCodeMode.value) {
      return const SizedBox.shrink();
    } else {
      return TextButton.icon(
        onPressed: () {
          _isDeployCodeMode.value = true;
          _emailError.value = '';
          _passwordError.value = '';
        },
        icon: Text(
          translate('Deploy with deploy code'),
          style: TextStyle(
            color: MyTheme.accent,
            fontSize: 14,
          ),
        ),
        label: Icon(
          Icons.arrow_forward,
          size: 16,
          color: MyTheme.accent,
        ),
      );
    }
  }

  Widget _buildDeployCodeMode(BuildContext context, bool isLoading) {
    return Column(
      children: [
        TextField(
          controller: _controller,
          decoration: InputDecoration(
            border: const OutlineInputBorder(),
            labelText: translate('Deploy Code'),
            hintText: translate('Enter 12-character code'),
            errorText: _errorTextEdit.isEmpty ? null : _errorTextEdit.value,
          ),
          inputFormatters: [
            FilteringTextInputFormatter.allow(RegExp(r'[a-zA-Z0-9]')),
            LengthLimitingTextInputFormatter(12),
            TextInputFormatter.withFunction((oldValue, newValue) {
              return TextEditingValue(
                text: newValue.text.toUpperCase(),
                selection: newValue.selection,
              );
            }),
          ],
          onChanged: (_) => _errorTextEdit.value = '',
          onSubmitted: isLoading
              ? null
              : (_) => _deployWithCodeRequest(_controller.text.trim()),
          enabled: !isLoading,
          style: const TextStyle(
            fontSize: 16,
            letterSpacing: 1.5,
          ),
          textCapitalization: TextCapitalization.characters,
        ),
        const SizedBox(height: 24),
        SizedBox(
          width: double.infinity,
          height: 50,
          child: ElevatedButton(
            onPressed: isLoading
                ? null
                : () {
                    _deployWithCodeRequest(_controller.text.trim());
                  },
            child: Text(
              translate('Confirm'),
              style: const TextStyle(fontSize: 16),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildDeployConfirmation(BuildContext context, bool isLoading) {
    final response = _deployResponse.value!;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Text(
          translate('Deployment Information'),
          style: const TextStyle(
            fontSize: 18,
            fontWeight: FontWeight.bold,
          ),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 20),
        Card(
          elevation: 8,
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                _buildInfoRow(translate('Team'), response.team),
                const SizedBox(height: 8),
                _buildInfoRow(translate('Email'), response.email),
                const SizedBox(height: 8),
                if (response.group.isNotEmpty)
                  _buildInfoRow(translate('Group'), response.group),
              ],
            ),
          ),
        ),
        const SizedBox(height: 24),
        Text(
          translate('deploy-confirm-tip'),
          style: const TextStyle(
            fontSize: 16,
            fontWeight: FontWeight.w500,
          ),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 24),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Expanded(
              child: OutlinedButton(
                onPressed: isLoading ? null : _cancelDeployment,
                style: OutlinedButton.styleFrom(
                  padding: const EdgeInsets.symmetric(vertical: 12),
                ),
                child: Text(translate('Cancel')),
              ),
            ),
            const SizedBox(width: 20),
            Expanded(
              child: ElevatedButton(
                onPressed: isLoading ? null : _confirmDeployment,
                style: ElevatedButton.styleFrom(
                  padding: const EdgeInsets.symmetric(vertical: 12),
                ),
                child: Text(translate('Confirm')),
              ),
            ),
          ],
        ),
      ],
    );
  }

  Widget _buildInfoRow(String label, String value) {
    return Row(
      children: [
        Text(
          '$label:',
          style: const TextStyle(
            fontSize: 14,
            fontWeight: FontWeight.w500,
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            value,
            style: const TextStyle(
              fontSize: 14,
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildAccountMode(BuildContext context, bool isLoading) {
    return Column(
      children: [
        LoginWidgetUserPass(
          usernameOrEmail: _emailController,
          pass: _passwordController,
          usernameOrEmailMsg:
              _emailError.value.isEmpty ? null : _emailError.value,
          passMsg: _passwordError.value.isEmpty ? null : _passwordError.value,
          isInProgress: isLoading,
          curOP: RxString(''),
          onLogin: _deployWithAccount,
          userFocusNode: FocusNode(),
          loginButtonText: 'Confirm',
        ),
      ],
    );
  }

  Widget _buildLoggedInMode(BuildContext context, bool isLoading) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Card(
          elevation: 8,
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              children: [
                if (gFFI.userModel.teamName.value.isNotEmpty)
                  _buildInfoRow(
                      translate('Team'), gFFI.userModel.teamName.value),
                if (gFFI.userModel.userName.value.isNotEmpty)
                  _buildInfoRow(
                      translate('User'), gFFI.userModel.userName.value),
              ],
            ),
          ),
        ),
        const SizedBox(height: 24),
        SizedBox(
          width: double.infinity,
          height: 50,
          child: ElevatedButton(
            onPressed: isLoading ? null : _deployToCurrentAccount,
            child: Text(
              translate('Confirm'),
              style: const TextStyle(fontSize: 16),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildErrorText(BuildContext context) {
    return Align(
      alignment: Alignment.centerLeft,
      child: SelectableText(
        gFFI.deployModel.error.value,
        style: TextStyle(
          color: Theme.of(context).colorScheme.error,
          fontSize: 12,
        ),
        textAlign: TextAlign.left,
      ).paddingOnly(top: 8, left: 12),
    );
  }
}
