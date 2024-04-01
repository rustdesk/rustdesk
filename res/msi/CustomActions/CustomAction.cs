using System;
using System.Collections.Generic;
using System.Linq;
using System.Diagnostics;
using System.Runtime.InteropServices;
using WixToolset.Dtf.WindowsInstaller;

namespace CustomActions
{
    public class CustomActions
    {
        [CustomAction]
        public static ActionResult CustomActionHello(Session session)
        {
            try
            {
                session.Log("================= Example CustomAction Hello");
                return ActionResult.Success;
            }
            catch (Exception e)
            {
                session.Log("An error occurred: " + e.Message);
                return ActionResult.Failure;
            }
        }

        [CustomAction]
        public static ActionResult RunCommandAsSystem(Session session)
        {
            try
            {
                ProcessStartInfo psi = new ProcessStartInfo
                {

                    FileName = "cmd.exe",
                    Arguments = "/c " + session["CMD"],
                    UseShellExecute = false,
                    WindowStyle = ProcessWindowStyle.Hidden,
                    Verb = "runas"
                };

                using (Process process = Process.Start(psi))
                {
                    process.WaitForExit();
                }

                return ActionResult.Success;
            }
            catch (Exception e)
            {
                session.Log("An error occurred: " + e.Message);
                return ActionResult.Failure;
            }
        }
    }
}
