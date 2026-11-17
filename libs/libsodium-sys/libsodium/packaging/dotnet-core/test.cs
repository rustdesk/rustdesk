using System;
using System.Runtime.InteropServices;

internal static class Program
{
    internal static int Main()
    {
        Console.WriteLine("sodium_version_string: {0}", Marshal.PtrToStringAnsi(sodium_version_string()));
        Console.WriteLine("sodium_library_version_major: {0}", sodium_library_version_major());
        Console.WriteLine("sodium_library_version_minor: {0}", sodium_library_version_minor());
        Console.WriteLine("sodium_library_minimal: {0}", sodium_library_minimal());
        int error = sodium_init();
        Console.WriteLine("sodium_init: {0}", error);
        if (error == 0)
        {
            randombytes_buf(out ulong buf, (UIntPtr)sizeof(ulong));
            Console.WriteLine("randombytes_buf: 0x'{0:X8}'", buf);
            Console.WriteLine("crypto_aead_aes256gcm_is_available: {0}", crypto_aead_aes256gcm_is_available());
        }
        return error == 0 ? 0 : 1;
    }

    [DllImport("libsodium", CallingConvention = CallingConvention.Cdecl)]
    private static extern int crypto_aead_aes256gcm_is_available();

    [DllImport("libsodium", CallingConvention = CallingConvention.Cdecl)]
    private static extern void randombytes_buf(out ulong buf, UIntPtr size);

    [DllImport("libsodium", CallingConvention = CallingConvention.Cdecl)]
    private static extern int sodium_init();

    [DllImport("libsodium", CallingConvention = CallingConvention.Cdecl)]
    private static extern int sodium_library_version_major();

    [DllImport("libsodium", CallingConvention = CallingConvention.Cdecl)]
    private static extern int sodium_library_minimal();

    [DllImport("libsodium", CallingConvention = CallingConvention.Cdecl)]
    private static extern int sodium_library_version_minor();

    [DllImport("libsodium", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr sodium_version_string();
}
