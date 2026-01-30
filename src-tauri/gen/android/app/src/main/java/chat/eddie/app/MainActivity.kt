package chat.eddie.app

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Switch from splash theme to regular app theme
    setTheme(R.style.Theme_eddie_chat)
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
