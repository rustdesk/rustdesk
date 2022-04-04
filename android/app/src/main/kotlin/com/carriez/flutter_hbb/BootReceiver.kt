package com.carriez.flutter_hbb

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import android.widget.Toast

class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if ("android.intent.action.BOOT_COMPLETED" == intent.action){
            val it = Intent(context,MainService::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            Toast.makeText(context, "RustDesk is Open", Toast.LENGTH_LONG).show();
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(it)
            }else{
                context.startService(it)
            }
        }
    }
}
