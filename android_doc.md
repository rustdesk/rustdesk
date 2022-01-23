# RustDesk 安卓端被控文档记录

### 1.获取屏幕录像

##### 原理 流程
MediaProjectionManager -> MediaProjection
-> VirtualDisplay -> Surface -> MediaCodec

- 获取mediaProjectionResultIntent
    - **必须activity**
    - activity获取mediaProjectionResultIntent
    - 会提示用户 “获取屏幕录制权限”

- 获取MediaProjection
    - **必须service**
    - 将mediaProjectionResultIntent 传递到后台服务
    - 通过后台服务获取MediaProjection

- 创建Surface(理解为一个buf)和Surface消费者
    - MediaCodec生成Surface传入VirtualDisplay的入参中
    - 设定编码等各类参数

- 获取VirtualDisplay(Surface 生产者)
    - 前台服务
    - MediaProjection createVirtualDisplay方法创建VirtualDisplay
    - 创建VirtualDisplay的入参之一是Surface
    - 需要设定正确的VirtualDisplay尺寸

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
- API大于O(26)时需要startForegroundService，且需要正确设置通知栏，
  新特性中使用ForegroundService不会被系统杀掉


##### 资料
- 关于 FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION 权限
  https://zhuanlan.zhihu.com/p/360356420

- 关于Notification 和 NotificationNotification
  https://stackoverflow.com/questions/47531742/startforeground-fail-after-upgrade-to-android-8-1
  https://developer.android.com/reference/android/support/v4/app/NotificationCompat.Builder.html#NotificationCompat.Builder(android.content.Context)

// TODO 使用 NotificationCompat 的区别

<hr>

### 2.获取控制
暂时可行的方案是使用安卓无障碍服务 参考droidVNC项目，但droidVNC的实现并不完善，droidVNC没有实现连续触控。

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
- 连续手势 https://developer.android.com/guide/topics/ui/accessibility/service?hl=zh-cn#continued-gestures

<hr>

### 3.获取音频输入
https://developer.android.google.cn/guide/topics/media/playback-capture?hl=zh-cn

目前谷歌只开放了Android10系统同步音频内录功能
10之前录音的时候会截取原本系统的音频输出
即 开启内录时候无法在手机上正常使用耳机扬声器输出

<hr>

### 其他
- Kotlin 与 compose 版本设置问题
    - https://stackoverflow.com/questions/67600344/jetpack-compose-on-kotlin-1-5-0
    - 在根目录的gradle中 设置两个正确对应版本

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
- 注意，原项目包名flutter_hbb 带有下划线，通过安卓的编译提示获得的命名方式为如上。

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

传递数据
Kotlin 中的 ByteArray 类 会在JVM中编译成为java的byte[]
byte[]通过jni传递到rust端时
通过jni.rs的方法
env.convert_byte_array()即可转化为Vec<u8>