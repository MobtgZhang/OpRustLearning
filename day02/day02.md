# 第02天：最小化内核

从独立式可执行程序开始，构建自己的内核，基于x86架构的操作系统。

## 引导启动

当启动电脑的时候，主板ROM内存储的固件依次运行：

+ 上电自检；
+ 可用内存(RAM)检测；
+ CPU以及其他硬件的预加载。

然后寻找一个可引导的存储介质，并开始引导启动其中的内核。

x86支持的两种标准：BIOS和UEFI标准。本文中主要涉及到BIOS启动

## BIOS启动

几乎所有的x86硬件系统均支持BIOS启动，这也包含基于UEFI用模拟BIOS的方式向后兼容的硬件系统。但是这种兼容行有时候也是BIOS引导启动最大的缺点，这意味着CPU必须先进入一个16位系统兼容的实模式方式。

当电脑启动时候，主板上特殊的闪存中存储的BIOS固件将被加载。BIOS估计将会上电自检、初始化硬件，然后它将寻找一个可引导的存储介质。随后将电脑的控制权被转交给引导程序(bootloader)：一段在存储介质开头512字节商都的程序片段。大多数引导程序长度均大于512字节的，所以在通常情况下，引导程序都会被切分位一段优先启动、长度不超过512字节、存储在介质开头的第一阶段引导程序和一段随后由其加载长度较长的、存储在其他位置的第二阶段引导程序。

引导程序必须决定内核的位置，并且将其加载到内存当中。引导程序还需要将CPU从16位的实模式切换到32位保护模式，最终切换到64位的长模式。在这个情况下所有的64位寄存器和整个主内存才能够被访问。
引导程序的第三个作用就是从BIOS查询特定的信息并将其传递到内核（例如查询和传递内存映射表）。

本程序中使用bootimage工具，它能够自动而且方便地位内核准备一个引导程序。

## Multiboot标准

1995年自由软件基金会颁布了一个开源的引导程序标准Multiboot。这个标准定义了引导程序和操作系统间的统一接口，所以任何适配Multiboot的引导程序都可以用于加载任何适配了Multiboot的操作系统。(例如Grub)

特别地，要编写一款适配Multiboot的内核，只需要在内核文件开头，插入称作Multiboot的数据片段。这让GRUB很容易引导任何操作系统，但Grub和Multiboot是也存在一些以下的问题

+ 只支持32位的保护模式，意味着引导之后仍然需要配置CPU切换到64位长模式；
+ 它们是被设计位精简引导程序而不是精简内核；
+ GRUB和Multiboot标准并没有被详细地注释，阅读相关文档需要一定经验；
+ 为了创建一个能够被引导的磁盘映像，我们在开发时必须安装GRUB：这加大了基于Windows或macOS开发内核的难度。

处于以上的考虑，决定不使用GRUB和Multiboot标准，bootimage工具也可以引导操作系统程序。

## 最小化内核

之前构建的独立二进制程序依然基于特性的操作系统平台并不是裸机上的程序。cargo在默认情况下会位特定的宿主系统构建源码。这并不是我们想要的结果，而是将程序编译位一个特定的目标系统。

## 安装Nightly Rust

使用`rustup`安装`nightly`版本的编译器：
```bash
rustup override add nightly
```

通过上述命令来选择当前目录使用的`nightly`版本的`Rust`。`Nightly`版本的编译器允许我们在源码开头插入特性标签自由选择并使用大量实验性的功能。

## 目标配置清单

通过`--target`参数，`cargo`支持不同的目标系统。这个目标系统可以使用一个目标三元租，描述CPU架构、平台供应者、操作系统和应用程序二进制接口。例如`x86_64-unknown-gnu`。


`x86_64-unknown-gnu`目标系统的配置清单项如下所示
```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true,
    "linker-flavor": "ld.lld",
    "linker": "rust-lld",
    "panic-strategy": "abort",
    "disable-redzone": true,
    "features": "-mmx,-sse,+soft-float"
}
```

一个配置清单中包含有多个配置项，大多数配置项都是LLVM需求的，将配置位特定平台生成的代码。
+ `data-layout`：定义了不同的整数、浮点数、指针类型的长度；
+ `target-XXXX-width`：`Rust`用作条件编译的配置项；
+ `pre-link-args`：制定了该向链接器传入的参数；
+ `llvm-target`：内容中`os`配置项值改为`none`；
+ `linker-flavor`和`linker`：使用跨平台的lld链接器，并和rust一起打包的；
+ `panic-strategy`：编译目标不支持`panic`时候的栈展开，选择直接在`panic`的时候终止；
+ `disable-redzone`：我们正在编写一个内核，所以我们应该同时处理中断。要安全地实现这一点，我们必须禁用一个与红区（redzone）有关的栈指针优化：因为此时，这个优化可能会导致栈被破坏。
+ `features`配置项被用来启用或禁用某个目标CPU特征（CPU feature）。通过在它们前面添加-号，我们将mmx和sse特征禁用；添加前缀+号，我们启用了soft-float特征。

mmx和sse特征决定了是否支持单指令多数据流（Single Instruction Multiple Data，SIMD）相关指令，这些指令常常能显著地提高程序层面的性能。然而，在内核中使用庞大的SIMD寄存器，可能会造成较大的性能影响：因为每次程序中断时，内核不得不储存整个庞大的SIMD寄存器以备恢复——这意味着，对每个硬件中断或系统调用，完整的SIMD状态必须存到主存中。由于SIMD状态可能相当大（512~1600个字节），而中断可能时常发生，这些额外的存储与恢复操作可能显著地影响效率。为解决这个问题，我们对内核禁用SIMD（但这不意味着禁用内核之上的应用程序的SIMD支持）。

禁用SIMD产生的一个问题是，x86_64架构的浮点数指针运算默认依赖于SIMD寄存器。我们的解决方法是，启用soft-float特征，它将使用基于整数的软件功能，模拟浮点数指针运算。

## 编译内核
```bash
cargo build --target x86_64-blog_os.json
```

但是这样会发生`can't find crate for "core"`的错误。

通常状况下，`core`库以预编译库的形式与`Rust`编译器同时发布，但是`core`只对宿主机有效，所以需要位这些系统重新编译整个`core`库。

## 使用`xbuild`工具

使用下面的命令安装它：
```bash
cargo install cargo-xbuild
```

这个工具依赖于`Rust`的源代码，可以使用以下的命令安装源代码
```bash
rustup component add rust-src
```

重新使用`xbuild`编译
```bash
cargo xbuild --target x86_64-blog_os.json
```

这样就可以为裸机编译内核了。


