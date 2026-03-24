package org.my_audio_codec;

import android.view.View;

import android.app.NativeActivity;
import android.os.Bundle;
import android.Manifest;
import android.content.pm.PackageManager;
import androidx.annotation.NonNull;
public class MainActivity extends NativeActivity {
       @Override
    protected void onCreate(Bundle saved) {
        super.onCreate(saved);
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.RECORD_AUDIO}, 123);
        }
    }

        @Override
        public void onWindowFocusChanged(boolean hasFocus) {
                super.onWindowFocusChanged(hasFocus);

                if (hasFocus) {
                        hideSystemUi();
                }
        }

        private void hideSystemUi() {
                View decorView = getWindow().getDecorView();
                decorView.setSystemUiVisibility(
                                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                                                | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                                                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                                                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                                                | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                                                | View.SYSTEM_UI_FLAG_FULLSCREEN);
        }
}
