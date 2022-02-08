# RustDesk 安卓端被控文档记录

### 1.获取屏幕录像

##### 原理 流程
MediaProjectionManager -> MediaProjection
-> VirtualDisplay -> Surface <- MediaCodec/ImageReader

- 获取mediaProjectionResultIntent
    - **必须activity**
    - activity获取mediaProjectionResultIntent
    - 会提示用户 “获取屏幕录制权限”

- 获取MediaProjection
    - **必须service**
    - 将mediaProjectionResultIntent 传递到后台服务
    - 通过后台服务获取MediaProjection

- 创建Surface(理解为一个buf)和Surface消费者
    - MediaCodec(使用内置编码器)或者ImageReader(捕获原始数据)生成Surface传入VirtualDisplay的入参中
    - 设定编码等各类参数

- 获取VirtualDisplay(Surface 生产者)
    - 前台服务
    - MediaProjection createVirtualDisplay方法创建VirtualDisplay
    - 创建VirtualDisplay的入参之一是Surface
    - 需要设定正确的VirtualDisplay尺寸

#####方案A 捕获原始数据传入rust进行编码
- 构建ImageReader生成Surface
- **注意**：安卓捕获到的数据是RGBA格式，暂无BRGA的输出，在rust端需要调用libyuv中相应的rgbatoi420方法
- 捕获到的数据存入一个bytearray，等待rust端调用获取

#####方案B 捕获原始数据传入rust进行编码 !等待完善！
- **自带的编码器无法直接控制流量，默认情况输出的帧率比较高，会造成网络堵塞延迟**
- 获取编码后的buf
    - 通过MediaCodec回调获取到可用的数据
- 通过jni传入Rust服务
    - 直接通过jni调用rust端的函数，将数据传递给video_service中

- 安卓VP9兼容性待测试
    - 目前测试2017年一台安卓7机器不支持vp9硬件加速
    - **安卓内置的编解码器并不一定是硬件解码**

##### 权限注意
```
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.RECORD_AUDIO"/>

<service
    ...
    android:foregroundServiceType="mediaProjection"/>
```
- API大于O(26/Android8.0)时需要startForegroundService，且需要正确设置通知栏，
  新特性中使用ForegroundService不会被系统杀掉


##### 资料
- 关于 FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION 权限
  https://zhuanlan.zhihu.com/p/360356420

- 关于Notification 和 NotificationNotification
  https://stackoverflow.com/questions/47531742/startforeground-fail-after-upgrade-to-android-8-1
  https://developer.android.com/reference/android/support/v4/app/NotificationCompat.Builder.html#NotificationCompat.Builder(android.content.Context)

<hr>

### 2.获取控制
暂时可行的方案是使用安卓无障碍服务 参考droidVNC项目，
**目前暂无可用连续输入的方案，暂时只能做到控制端鼠标滑动抬起鼠标后才能发送这组控制到安卓端**

#### 无障碍服务获取权限
- https://developer.android.com/guide/topics/ui/accessibility/service?hl=zh-cn#manifest
- 清单文件
  ```
  <application>
  <service android:name=".MyAccessibilityService"
      android:permission="android.permission.BIND_ACCESSIBILITY_SERVICE"
      android:label="@string/accessibility_service_label">
    <intent-filter>
      <action android:name="android.accessibilityservice.AccessibilityService" />
    </intent-filter>
  </service>
  </application>
  ```
- 创建一个单独的xml文件，用于无障碍服务配置
  ```
  // 首先清单文件中增加文件地址
  <service android:name=".MyAccessibilityService">
    ...
    <meta-data
      android:name="android.accessibilityservice"
      android:resource="@xml/accessibility_service_config" />
  </service>
  // 然后在此位置添加xml
  // <project_dir>/res/xml/accessibility_service_config.xml
  <accessibility-service xmlns:android="http://schemas.android.com/apk/res/android"
    ...
    android:canPerformGestures="true" // 这里最关键
  />
  ```
- ~~连续手势 https://developer.android.com/guide/topics/ui/accessibility/service?hl=zh-cn#continued-gestures~~

#### knox.remotecontrol 三星手机 专用控制方式

https://docs.samsungknox.com/devref/knox-sdk/reference/com/samsung/android/knox/remotecontrol/package-summary.html


<hr>

### 3.获取音频输入
https://developer.android.google.cn/guide/topics/media/playback-capture?hl=zh-cn

**仅安卓10或更高可用**
目前谷歌只开放了Android10及以上系统同步音频内录功能
10之前录音的时候会截取原本系统的音频输出
即 开启内录时候无法在手机上正常使用耳机扬声器输出
且普通应用的声音默认不会被捕获

**安卓10音频输入原理**
- 音频权限相当于是MediaProjection的附属产物
- 只有在成功获取MediaProjection，开启了ForegroundService才能使用
- 相比于AudioRecord普通用法使用，将setAudioSource改为setAudioPlaybackCaptureConfig，这里的AudioPlaybackCaptureConfiguration的构建需要使用到之前成功获取的MediaProjection
<br>
- **一些注意事项**
  - 使用AudioFormat.ENCODING_PCM_FLOAT，数值范围[-1,1]的32位浮点数据，对应了rust端opus编码器的输入格式。
  - libopus库中使用的opus_encode_float，对于输入的音频数据长度有一定要求，安卓端输出的包过大需要分批发送
    - https://stackoverflow.com/questions/46786922/how-to-confirm-opus-encode-buffer-size
    - https://docs.rs/audiopus_sys/0.2.2/audiopus_sys/fn.opus_encode_float.html
    - > For example, at 48 kHz the permitted values are 120, 240, 480, 960, 1920, and 2880. 
  - 安卓11自带了opus输出，几年后或许可用


<hr>

### Rust JVM 互相调用

rust端 引入 jni crate
https://docs.rs/jni/0.19.0/jni/index.html

Kotlin端
类中通过init{} 引入lib的调用
```kotlin
class Main{
  init{
    System.loadLibrary("$libname")
  }
}
```

Rust端
使用jni规则进行函数命名
```rust
pub unsafe extern "system" fn Java_com_carriez_flutter_1hbb_MainActivity_init(
    env: JNIEnv,
    class: JClass,
    ctx:JObject,
){

}
```
- 注意，原项目包名flutter_hbb 带有下划线，通过安卓的编译提示获得的命名方式为如上的`..._1hbb...`。

- 使用jni的时候，如果不捕捉错误会出现无输出崩溃的情况
- 将安卓的对象实例（Context）在init的过程中传入rust端，  
context通过env.new_global_ref()变成全局引用
env.get_java_vm()获取到jvm
  - 原理上 Rust端通过类找静态方法也可行，但在kotlin端测试失败，会遇到类名找不到，类静态方法找不到等问题，目前仅使用绑定具体context对象即可。
- 将jvm和context 固定到全局变量中等待需要时候引用
- 使用时，需要确保jvm与当前的线程绑定
jvm.attach_current_thread_permanently()
- 然后通过jvm获得env
jvm.get_env()
- 通过env.call_method()方法传入context.as_obj()使用对象的方法
- ByteArray(Kotlin) 
  - 会在JVM中编译成为java的byte[]
  - rust端通过jbytearray接收，通过jni.rs的方法
env.convert_byte_array()即可转化为Vec数组

- FloatArray(Kotlin)
  - JAVA中的float[]
  - jni.rs中使用get_float_array_region方法写入到新buf中

- ByteBuffer(Kotlin/Java)
  - 
- 关于 sig 一些用例 https://imzy.vip/posts/55704/
  - (I)V ： input:Int,output:Void
  - (I)Ljava/nio/ByteBuffer ： input:Int,output:ByteBuffer. convert method:env.get_direct_buffer_address()
  - ()[B : input:void,output:byte[](java) == ByteArray(kotlin) == jbytearray(jni env 中有对应解析方法到Vec\<u8\>)
  - call java的方法时，使用JValue枚举定义java变量


<hr>

### UI交互相关
收到无密码登录请求时，1.通知flutterUI页面问询用户2.通知栏通知用户
- 否
  - 返回给rust端否 java端不做处理
- 是
  - 返回给rust端是 java端开始采集音视频

收到有密码登录请求，rust端可以自动判断
- 否
  - rust端自动处理返回密码错误
- 是
  - 通知java端 java端开始采集 同时在通知推送栏中推送消息

#### 服务开启与关闭
1.start listen
安卓端用户手动开始服务监听 开启service
获取视频权限 成功后 通知flutter将图标状态转为已开启 开启rust端的start_all()
然后就可以被其他人请求连接

2.login request
验证成功的请求，
安卓端开启视频 音频 输入的采集
通知rust端logon response

3.client close
rust端会自动结束
rust端发送结束指令给安卓端
安卓端停止各项采集 但服务依然开启

4.server close
4-1
close conn 
用户点击断开连接
安卓端停止各项采集
发送close指令给rust让rust关闭这个conn

4-1
close totally 
如果当前有连接则问是否断开
是则先执行一遍4-1
然后关闭整个service


服务端主动关闭服务
Config::set_option("stop_service","Y")
服务端再次启动服务
Config::set_option("stop_service","")


### TODO 
完善CM 当前连接的状态 控制音频和输入等开关 断开连接等功能
横屏模式
首次登录不显示id密码
安卓前后分离的问题 通过IPC或者广播解耦

### 关于安卓的service和进程
实际测试 安卓7和安卓11表现不同 同一个apk下若有多个activity或service
安卓7 关闭activity后所有的服务都会强制关闭 可能是锤子手机特有

安卓8.1 和7类似  且安卓8.1和7录屏权限比较宽松 只需要获取一次 不需要每次播放都要获取录屏权限

*安卓7/8.1关闭activity 后就关闭service 可能是锤子OS的特质

理论上 非bind启动的service可以脱离activity运行 就像三星安卓11上测试的情况

安卓11 关闭activity后service可以单独运行 可能由于前台应用可以持续维持
  再次进入程序新的activity会共用之前在内存中的so程序

安卓Service运行在主线程！才能实现脱离activity独立运行

>只有在内存过低且必须回收系统资源以供拥有用户焦点的 Activity 使用时，Android 系统才会停止服务。如果将服务绑定到拥有用户焦点的 Activity，则它其不太可能会终止；如果将服务声明为在前台运行，则其几乎永远不会终止。如果服务已启动并长时间运行，则系统逐渐降低其在后台任务列表中的位置，而服务被终止的概率也会大幅提升—如果服务是启动服务，则您必须将其设计为能够妥善处理系统执行的重启。如果系统终止服务，则其会在资源可用时立即重启服务，但这还取决于您从 onStartCommand() 返回的值。

>如服务文档中所述，您可以创建同时具有已启动和已绑定两种状态的服务。换言之，您可以通过调用 startService() 来启动服务，让服务无限期运行，您也可以通过调用 bindService() 让客户端绑定到该服务。
如果您确实允许服务同时具有已启动和已绑定状态，那么服务启动后，系统不会在所有客户端均与服务取消绑定后销毁服务，而必须由您通过调用 stopSelf() 或 stopService() 显式停止服务。
尽管您通常应实现 onBind() 或 onStartCommand()，但有时也需要同时实现这两种方法。例如，音乐播放器可能认为，让其服务无限期运行并同时提供绑定很有用处。如此一来，Activity 便可启动服务来播放音乐，并且即使用户离开应用，音乐播放也不会停止。然后，当用户返回应用时，Activity 便能绑定到服务，重新获得播放控制权。

onRebind()

[进程和应用生命周期](https://developer.android.com/guide/components/activities/process-lifecycle?hl=zh-cn)

[进程和线程概览](https://developer.android.com/guide/components/processes-and-threads?hl=zh-cn)

[绑定服务概览](https://developer.android.com/guide/components/bound-services?hl=zh-cn)

Service持久化与绑定具体操作 [已测试安卓7.1以上系统特性相同]
1.前台服务 service中调用startForeground，启用持久化Service需要保证至少一次通过startForegroundService/startService 启动了Service且在Service中主动startForeground，可以通过intent传参指定一次init操作，在init的过程中startForeground，最关键的操作就是startForeground
即 通过startService 调用过的Service并且Service中调用过startForeground的Service就是持久化的前台服务，服务不会被系统kill
2.通过使用bindService将Activity与Service绑定 使用unbindService在onDestroy中解绑，如果不解绑会造成对Service的引用泄漏引发错误。可以在Activity中的onCreate中进行绑定，也可以根据需求按需手动进行绑定，bindService startService的先后顺序无所谓
*只要注意至少一次对Service调用过startForegroundService/startService*

关于startForegroundService
https://developer.android.com/about/versions/oreo/background?hl=zh-cn#services
尽量使用startForegroundService传递start命令 注意首次使用的时候需要在5秒内在服务中主动调用startService

改成bindService逻辑
直接在activity onCreate时候进行绑定 onDestroy时解绑（注意判空），绑定的时候进行一些判断。如果已存在服务会话则恢复之前的情况。如果不存在不服务会话则等待需要的时候再启动前台服务

<hr>

### 其他
- Kotlin 与 compose 版本设置问题
    - https://stackoverflow.com/questions/67600344/jetpack-compose-on-kotlin-1-5-0
    - 在根目录的gradle中 设置两个正确对应版本
- 如果开发环境中安装了超过一种NDK版本，则会需要在app的build.gradle中指定NDK版本
  ```
  // build.gradle in app
  android {
    ...
    compileSdkVersion 30
    ndkVersion '22.1.7171670' 
    ...
  ```