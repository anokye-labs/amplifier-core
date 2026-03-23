# Amplifier.FFI.Runtime

Native runtime package containing platform-specific `amplifier_ffi` shared libraries for P/Invoke interop with [amplifier-core](https://github.com/anokye-labs/amplifier-core).

## Supported platforms

| RID        | Library              |
|------------|----------------------|
| win-x64    | amplifier_ffi.dll    |
| linux-x64  | libamplifier_ffi.so  |

## Usage

Reference this package from your .NET project. The build system copies the correct native library to your output directory automatically.

```xml
<PackageReference Include="Amplifier.FFI.Runtime" Version="0.1.0" />
```
