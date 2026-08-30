import { abilityAccessCtrl, common, Context, Permissions, wantAgent, WantAgent } from '@kit.AbilityKit';
import { audio } from '@kit.AudioKit';
import { deviceInfo, pasteboard } from '@kit.BasicServicesKit';
import { backgroundTaskManager } from '@kit.BackgroundTasksKit';
import { i18n } from '@kit.LocalizationKit';
import { display, window } from '@kit.ArkUI';
import {
  AbilityAware,
  AbilityPluginBinding,
  FlutterPlugin,
  FlutterPluginBinding,
  MethodCall,
  MethodCallHandler,
  MethodChannel,
  MethodResult,
} from '@ohos/flutter_ohos';

const METHOD_CHANNEL_NAME = 'top.frankhan.resk.flutter/platform';

export default class RustDeskPlatformBridge implements FlutterPlugin, MethodCallHandler, AbilityAware {
  private channel: MethodChannel | null = null;
  private context: Context | null = null;
  private abilityContext: common.UIAbilityContext | null = null;
  private continuousTaskId: number | null = null;
  private continuousTaskStarting: boolean = false;
  private readonly systemPasteboard: pasteboard.SystemPasteboard = pasteboard.getSystemPasteboard();

  getUniqueClassName(): string {
    return 'RustDeskPlatformBridge';
  }

  onAttachedToEngine(binding: FlutterPluginBinding): void {
    this.context = binding.getApplicationContext();
    this.channel = new MethodChannel(binding.getBinaryMessenger(), METHOD_CHANNEL_NAME);
    this.channel.setMethodCallHandler(this);
  }

  onDetachedFromEngine(binding: FlutterPluginBinding): void {
    this.channel?.setMethodCallHandler(null);
    this.channel = null;
    this.context = null;
  }

  onAttachedToAbility(binding: AbilityPluginBinding): void {
    this.abilityContext = binding.getAbility().context;
  }

  onDetachedFromAbility(): void {
    // An explicitly started host survives Ability recreation, so its continuous-task indicator
    // must remain active until the local Stop service action tears both down.
    this.abilityContext = null;
  }

  onMethodCall(call: MethodCall, result: MethodResult): void {
    try {
      switch (call.method) {
        case 'getFilesDir':
          result.success(this.abilityContext?.filesDir ?? this.context?.filesDir ?? '');
          return;
        case 'getDeviceType':
          result.success(deviceInfo.deviceType);
          return;
        case 'getSystemLocale':
          result.success(i18n.System.getSystemLocale());
          return;
        case 'getDefaultDisplayInfo':
          const displayInfo = display.getDefaultDisplaySync();
          result.success({
            'displayId': displayInfo.id,
            'width': displayInfo.width,
            'height': displayInfo.height,
          });
          return;
        case 'ensureMicrophonePermission':
          this.ensureMicrophonePermission(result);
          return;
        case 'getClipboardText':
          this.getClipboardText(result);
          return;
        case 'setClipboardText':
          this.setClipboardText(call, result);
          return;
        case 'getAudioInputDevices':
          result.success(this.getAudioDevices(audio.DeviceFlag.INPUT_DEVICES_FLAG));
          return;
        case 'getAudioOutputDevices':
          result.success(this.getAudioDevices(audio.DeviceFlag.OUTPUT_DEVICES_FLAG));
          return;
        case 'selectAudioInputDevice':
          this.selectAudioInputDevice(call, result);
          return;
        case 'prepareWindow':
          this.withMainWindow(result, async (mainWindow: window.Window): Promise<Object | null> => {
            const titleButtonWidth = mainWindow.getTitleButtonRect().width;
            mainWindow.setWindowDecorVisible(false);
            mainWindow.setWindowTitleButtonVisible(false, false, false);
            mainWindow.setWindowTitleMoveEnabled(false);
            return titleButtonWidth;
          });
          return;
        case 'isWindowMaximized':
          this.withMainWindow(result, async (mainWindow: window.Window): Promise<Object | null> =>
            this.isMaximized(mainWindow));
          return;
        case 'minimizeWindow':
          this.withMainWindow(result, async (mainWindow: window.Window): Promise<Object | null> => {
            await mainWindow.minimize();
            return null;
          });
          return;
        case 'toggleMaximizeWindow':
          this.withMainWindow(result, async (mainWindow: window.Window): Promise<Object | null> => {
            if (this.isMaximized(mainWindow)) {
              await mainWindow.recover();
              return false;
            }
            await mainWindow.maximize(window.MaximizePresentation.EXIT_IMMERSIVE);
            return true;
          });
          return;
        case 'startMovingWindow':
          this.withMainWindow(result, async (mainWindow: window.Window): Promise<Object | null> => {
            await mainWindow.startMoving();
            return null;
          });
          return;
        case 'setKeepScreenOn':
          this.withMainWindow(result, async (mainWindow: window.Window): Promise<Object | null> => {
            const enabled = (call.argument('enabled') as boolean | null) ?? false;
            await mainWindow.setWindowKeepScreenOn(enabled);
            return null;
          });
          return;
        case 'startContinuousTask':
          this.startContinuousTask(result);
          return;
        case 'stopContinuousTask':
          this.stopContinuousTask(result);
          return;
        case 'closeWindow':
          if (this.abilityContext === null) {
            result.error('OHOS_WINDOW_CONTEXT_MISSING', 'HarmonyOS UIAbility context is unavailable', null);
            return;
          }
          this.abilityContext.terminateSelf()
            .then(() => result.success(null))
            .catch((error: Error) => result.error('OHOS_WINDOW_ERROR', this.errorMessage(error), null));
          return;
        default:
          result.notImplemented();
      }
    } catch (error) {
      result.error('OHOS_PLATFORM_ERROR', this.errorMessage(error), null);
    }
  }

  private withMainWindow(
    result: MethodResult,
    action: (mainWindow: window.Window) => Promise<Object | null>,
  ): void {
    const context = this.abilityContext ?? this.context;
    if (context === null) {
      result.error('OHOS_WINDOW_CONTEXT_MISSING', 'HarmonyOS application context is unavailable', null);
      return;
    }
    window.getLastWindow(context)
      .then((mainWindow: window.Window) => action(mainWindow))
      .then((value: Object | null) => result.success(value))
      .catch((error: Error) => result.error('OHOS_WINDOW_ERROR', this.errorMessage(error), null));
  }

  private ensureMicrophonePermission(result: MethodResult): void {
    if (this.abilityContext === null) {
      result.error('OHOS_AUDIO_CONTEXT_MISSING', 'HarmonyOS UIAbility context is unavailable', null);
      return;
    }
    const permission: Permissions = 'ohos.permission.MICROPHONE';
    abilityAccessCtrl.createAtManager()
      .requestPermissionsFromUser(this.abilityContext, [permission])
      .then((data) =>
        result.success(data.authResults.length > 0 && data.authResults[0] === 0))
      .catch((error: Error) => result.error('OHOS_AUDIO_PERMISSION_ERROR', this.errorMessage(error), null));
  }

  private getClipboardText(result: MethodResult): void {
    this.systemPasteboard.getData()
      .then((data: pasteboard.PasteData) => result.success(data.getPrimaryText()))
      .catch((error: Error) => result.error('OHOS_CLIPBOARD_READ_ERROR', this.errorMessage(error), null));
  }

  private setClipboardText(call: MethodCall, result: MethodResult): void {
    const text = (call.argument('text') as string | null) ?? '';
    this.systemPasteboard.setData(pasteboard.createPlainTextData(text))
      .then(() => result.success(null))
      .catch((error: Error) => result.error('OHOS_CLIPBOARD_WRITE_ERROR', this.errorMessage(error), null));
  }

  private startContinuousTask(result: MethodResult): void {
    if (this.abilityContext === null) {
      result.error('OHOS_BACKGROUND_CONTEXT_MISSING', 'HarmonyOS UIAbility context is unavailable', null);
      return;
    }
    if (this.continuousTaskId !== null || this.continuousTaskStarting) {
      result.success(null);
      return;
    }
    this.continuousTaskStarting = true;
    const context = this.abilityContext;
    const wantAgentInfo: wantAgent.WantAgentInfo = {
      wants: [{
        bundleName: 'top.frankhan.resk.flutter',
        abilityName: 'EntryAbility',
      }],
      actionType: wantAgent.OperationType.START_ABILITY,
      requestCode: 0,
      wantAgentFlags: [wantAgent.WantAgentFlags.UPDATE_PRESENT_FLAG],
    };
    wantAgent.getWantAgent(wantAgentInfo)
      .then((agent: WantAgent) => backgroundTaskManager.startBackgroundRunning(
        context,
        ['dataTransfer', 'audioRecording'],
        agent,
      ))
      .then((task: backgroundTaskManager.ContinuousTaskNotification) => {
        this.continuousTaskId = task.continuousTaskId ?? null;
        this.continuousTaskStarting = false;
        result.success(null);
      })
      .catch((error: Error) => {
        this.continuousTaskStarting = false;
        result.error('OHOS_BACKGROUND_START_ERROR', this.errorMessage(error), null);
      });
  }

  private stopContinuousTask(result: MethodResult): void {
    if (this.abilityContext === null) {
      result.error('OHOS_BACKGROUND_CONTEXT_MISSING', 'HarmonyOS UIAbility context is unavailable', null);
      return;
    }
    this.stopContinuousTaskInternal(this.abilityContext)
      .then(() => result.success(null))
      .catch((error: Error) =>
        result.error('OHOS_BACKGROUND_STOP_ERROR', this.errorMessage(error), null));
  }

  private async stopContinuousTaskInternal(context: common.UIAbilityContext): Promise<void> {
    const taskId = this.continuousTaskId;
    this.continuousTaskId = null;
    this.continuousTaskStarting = false;
    if (taskId === null) {
      return;
    }
    await backgroundTaskManager.stopBackgroundRunning(context, taskId);
  }

  private isMaximized(mainWindow: window.Window): boolean {
    const status = mainWindow.getWindowStatus();
    return status === window.WindowStatusType.MAXIMIZE || status === window.WindowStatusType.FULL_SCREEN;
  }

  private getAudioDevices(flag: audio.DeviceFlag): Array<string> {
    const routingManager = audio.getAudioManager().getRoutingManager();
    const descriptors = routingManager.getDevicesSync(flag);
    const names: Array<string> = [];
    descriptors.forEach((descriptor: Readonly<audio.AudioDeviceDescriptor>) => {
      const name = this.audioDeviceName(descriptor);
      if (!names.includes(name)) {
        names.push(name);
      }
    });
    return names;
  }

  private selectAudioInputDevice(call: MethodCall, result: MethodResult): void {
    const requested = call.argument('device') as string | null;
    const deviceName = requested ?? '';
    const audioManager = audio.getAudioManager();
    const sessionManager = audioManager.getSessionManager();
    if (deviceName.length === 0) {
      sessionManager.clearSelectedMediaInputDevice()
        .then(() => result.success(null))
        .catch((error: Error) => result.error('OHOS_AUDIO_ERROR', this.errorMessage(error), null));
      return;
    }
    const descriptors = audioManager.getRoutingManager().getDevicesSync(audio.DeviceFlag.INPUT_DEVICES_FLAG);
    const selected = descriptors.find((descriptor: Readonly<audio.AudioDeviceDescriptor>) =>
      this.audioDeviceName(descriptor) === deviceName);
    if (selected === undefined) {
      result.error('OHOS_AUDIO_DEVICE_NOT_FOUND', `Audio input device is unavailable: ${deviceName}`, null);
      return;
    }
    sessionManager.selectMediaInputDevice(selected)
      .then(() => result.success(null))
      .catch((error: Error) => result.error('OHOS_AUDIO_ERROR', this.errorMessage(error), null));
  }

  private audioDeviceName(descriptor: Readonly<audio.AudioDeviceDescriptor>): string {
    if (descriptor.displayName.length > 0) {
      return descriptor.displayName;
    }
    if (descriptor.name.length > 0) {
      return descriptor.name;
    }
    return `Audio device ${descriptor.id}`;
  }

  private errorMessage(error: Object): string {
    return error instanceof Error ? error.message : `${error}`;
  }
}
