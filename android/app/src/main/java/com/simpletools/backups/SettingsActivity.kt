package com.simpletools.backups

import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity

class SettingsActivity : AppCompatActivity() {

    private lateinit var peerAddrInput: EditText
    private lateinit var watchDirsInput: EditText
    private lateinit var btnSave: Button

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)

        supportActionBar?.setDisplayHomeAsUpEnabled(true)
        title = "Settings"

        peerAddrInput = findViewById(R.id.proxy_addr_input)
        watchDirsInput = findViewById(R.id.watch_dirs_input)
        btnSave = findViewById(R.id.btn_save)

        val prefs = getSharedPreferences("settings", MODE_PRIVATE)
        peerAddrInput.hint = "desktop host:port"
        peerAddrInput.setText(
            prefs.getString("peer_addr", null)
                ?: prefs.getString("proxy_addr", "")
        )

        val defaultDirs =
            """[{"path":"/sdcard/DCIM","patterns":["*.jpg","*.jpeg","*.png","*.heic","*.mp4","*.gif"]},{"path":"/sdcard/Pictures","patterns":["*.jpg","*.jpeg","*.png","*.heic","*.gif"]},{"path":"/sdcard/Download","patterns":["*.pdf","*.jpg","*.jpeg","*.png","*.mp4"]}]"""
        watchDirsInput.setText(prefs.getString("watch_dirs", defaultDirs))

        btnSave.setOnClickListener { save() }
    }

    override fun onSupportNavigateUp(): Boolean {
        finish()
        return true
    }

    private fun save() {
        val peerAddr = peerAddrInput.text.toString().trim()
        val watchDirs = watchDirsInput.text.toString().trim()

        if (watchDirs.isNotEmpty()) {
            try {
                BackupNative.setWatchDirs(watchDirs)
            } catch (e: Exception) {
                Toast.makeText(this, "Invalid watch dirs: ${e.message}", Toast.LENGTH_LONG).show()
                return
            }
        }

        if (peerAddr.isNotEmpty()) {
            BackupNative.setPeerAddr(peerAddr)
        }

        getSharedPreferences("settings", MODE_PRIVATE).edit().apply {
            putString("peer_addr", peerAddr)
            putString("watch_dirs", watchDirs)
            apply()
        }

        Toast.makeText(this, "Settings saved", Toast.LENGTH_SHORT).show()
        finish()
    }
}
