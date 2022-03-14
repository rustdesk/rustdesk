# virtual display

Virtual display may be used on computers that do not have a monitor.

[Development reference](https://github.com/pavlobu/deskreen/discussions/86)

## windows

### win10

Win10 provides [Indirect Display Driver Model](https://msdn.microsoft.com/en-us/library/windows/hardware/mt761968(v=vs.85).aspx).

This lib uses [this project](https://github.com/fufesou/RustDeskIddDriver) as the driver.


**NOTE**: Versions before Win10 1607. Try follow [this method](https://github.com/fanxiushu/xdisp_virt/tree/master/indirect_display).


#### tested platforms

- [x] 19041
- [x] 19043

### win7

TODO

[WDDM](https://docs.microsoft.com/en-us/windows-hardware/drivers/display/windows-vista-display-driver-model-design-guide).

## X11

## OSX
