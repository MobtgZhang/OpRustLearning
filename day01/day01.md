# 第01天：独立式可执行程序

第一步用Rust编写操作系统，是在不连接标准库的前提下，创建独立的Rust可执行文件，并且无需操作系统的支撑在裸机上运行。

## 禁止使用标准库
在默认情况下，所有的Rust包俊辉链接标准库，然而标准库是依赖于操作系统功能的，并且与Rust的C标准库实现库相关联，它也是和操作系统紧密交互的。所以为了不使用操作系统底层库，所以必须禁用标准库自动引用。

使用`cargo`创建一个`blog_os`项目，最后形成的目录如下所示
```text
blog_os
├── Cargo.toml
└── src
    └── main.rs
```
其中`Cargo.toml`文件包含了包的配置，`src/main.rs`文件中则是根模块和`main`函数，然后在`target/debug`文件夹当中找到编译好的`blog_os`二进制文件。

### 增加`no_std`属性
默认情况下，Rust的标准库是隐式引用，使用以下的方式可以禁止使用标准库
```rust
#![no_std]

fn main(){
    println!();
}
```

但是在这种情况下，使用标准输出`println!`是基于标准库的，这个是由操作系统提供的，移除这段代码之后，再进行编译
```bash
cargo build
```

但是这样编译缺少一个`#[panic_handler]`函数和一个语言项`eh_personality`。

### 实现panic处理函数

`panic_handler`属性被用于定义一个函数；在程序`panic`时候，则这个程序会被调用到。这里需要定义自己的`panic`处理函数。
```rust
#![no_std]
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info:&PanicInfo) -> ! {
    loop {}
}

```

注意，类型为`PanicInfo`的参数包含了`panic`发生的文件名、代码行数和可选的错误信息。这个函数从不返回，所以一般被标记为发散函数。发散函数的返回类型称为`Never`类型，记作`!`。目前这个函数能做的事情很少，所以只需要编写一个无线循环`loop {}`。

### `eh_personality`语言项

语言项是一些编译器需求的特殊函数或者类型。但是语言项是高度不稳定的语言细节实现，并不经过编译器类型检查（甚至不确保参数类型是否正确）。

`eh_personality`语言项标记的函数将被用于实现栈展开，使用标准库的情况下，当`panic`发生的时候，Rust将使用栈展开运行在栈上所有变量的析构函数，这样确保所有使用过的内存会被释放，从而允许调用父进程捕获`panic`，处理并且继续运行。然而栈展开过程是一个非常复杂的过程，Linux的libunwind或者Windows的结构化异常处理，通常需要依赖于操作系统的库，此处并不在自己编写的操作系统中使用到它。

### 禁止栈展开

特别在其他情况下，栈展开并不是迫切需求的功能，故而Rust提供了`panic`时终止的选项。这个选项能够禁用栈展开相关的标志信息生成，同时也可以缩小生成的二进制程序的长度。有很多方法打开这个选项，此处使用到以下的方式加入到文件`Cargo.toml`：
```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "release

```

以上设置的方法可以将`dev`配置和`release`配置的`panic`策略设置为`abort`。其中`dev`配置适用于`cargo bulid`，而`release`配置适用于`cargo build --release`。现在编译器应该不再要求我们提供`eh_personality`语言项实现。

编译之后出现了一个新的错误，即需要一个`start`语言项，这需要定义程序中的入口函数。

现在需要告诉Rust编译器不使用预定义的入口点，并添加`#![no_main]`属性告诉编译器这样的一个选项。移除`main`函数之后，显然已经没有底层已有的库调用它，`main`函数将不会被运行。为了重写操作系统的入口点，所以编写了一个`_start`函数

```rust
#[no_mangle]
pub extern "C" fn _start() -> !{
    loop {}
}
```

这里使用到了`no_mangle`标记这个函数对它禁用名称重整，确保Rust编译器输出一个名为`_start`函数，否则生成一个其他函数导致无法让连接器正确辨别。`extern "C"`也告诉了编译器使用的是C语言的调用约定而不是Rust语言调用约定，`_start`是大多数系统默认使用这个名字作为入口点名称。此函数返回值类型为`!`，是一个发散函数。

但是编译这段程序之后会出现链接器错误。

### 链接器错误

链接器值将生成的目标文件组合成为一个可执行文件，不同类型的系统会有各自的链接器，抛出不同的错误，但是根本原因是一致的：链接器默认配置嘉定程序依赖于C语言的运行时环境，但是程序并不是依赖它的。此时，需要告诉链接器不应该包含C语言的运行环境，可以选择提供特定的链接器参数，也可以编译位裸机目标。

例如当前的操作系统为`x86_64-unknown-linux-gnu`，Rust编译器尝试为当前的操作系统三元组编译，错误会导致链接器错误，我们需要选择一个底层没有操作系统的运行环境（裸机），这里选择`thumbv7em-none-eabihf`描述一个ARM嵌入式系统，这个需要用`rustup`安装这个目标：

```bash
rustup target add thumbv7em-none-eabihf
```

这样就可以位这个目标构建独立式可执行程序了
```bash
cargo build --target thumbv7em-none-eabihf
```

其中`--target`参数表示的是目标系统进行交叉编译程序，目标不包括当前操作系统所以就不会尝试链接C语言运行环境。

还有一种解决的办法就是，不更换编译目标，而是传送特定的链接器参数尝试修复错误，可以在`.cargo/config`文件中添加以下的参数进行编译处理：
```toml
# in .cargo/config

[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-nostartfiles"]

[target.'cfg(target_os = "windows")']
rustflags = ["-C", "link-args=/ENTRY:_start /SUBSYSTEM:console"]

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-args=-e __start -static -nostartfiles"]
```

虽然以上的方式可以解决面向对个系统编译独立式可执行程序，但是这可能不是一个好的途径，原因是可执行程序仍然需要其他准备（例如`_start`函数调用前一个加载完毕的栈），特别是不使用C语言运行环境的前提下，可能这些并没有全部完成，可能会导致段错误。











