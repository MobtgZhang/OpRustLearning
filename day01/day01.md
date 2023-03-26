# 第一天：独立式可执行程序

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




