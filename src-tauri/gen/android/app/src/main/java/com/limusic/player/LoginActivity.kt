package com.limusic.player

import android.annotation.SuppressLint
import android.graphics.Bitmap
import android.os.Bundle
import android.webkit.CookieManager
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback

/**
 * Google sign-in in a WebView of its own, the Android counterpart of the desktop login window
 * (`src-tauri/src/session.rs`). When the redirect lands on music.youtube.com with a session, the
 * cookie string goes to Rust, which signs in exactly as it does on desktop.
 */
class LoginActivity : ComponentActivity() {
  companion object {
    const val EXTRA_ADD_ACCOUNT = "add_account"
    private const val LOGIN_URL =
      "https://accounts.google.com/ServiceLogin?service=youtube&continue=https://music.youtube.com/"
    private const val ADD_ACCOUNT_URL =
      "https://accounts.google.com/AddSession?service=youtube&continue=https://music.youtube.com/"
  }

  private lateinit var web: WebView
  private var done = false

  @SuppressLint("SetJavaScriptEnabled")
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    val cookies = CookieManager.getInstance()
    cookies.setAcceptCookie(true)
    val addAccount = intent.getBooleanExtra(EXTRA_ADD_ACCOUNT, false)
    if (addAccount) {
      // Sign the WebView out first so the new account is the only session it holds; the app's
      // saved accounts live in its own database, not here.
      cookies.removeAllCookies(null)
      cookies.flush()
    }

    web = WebView(this)
    cookies.setAcceptThirdPartyCookies(web, true)
    web.settings.javaScriptEnabled = true
    web.settings.domStorageEnabled = true
    // Google refuses sign-in from user agents that admit to being an embedded WebView.
    web.settings.userAgentString = web.settings.userAgentString.replace("; wv)", ")")
    web.webViewClient = object : WebViewClient() {
      override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) = check(url)

      override fun onPageFinished(view: WebView, url: String) = check(url)

      override fun onReceivedError(
        view: WebView,
        request: WebResourceRequest,
        error: WebResourceError,
      ) {
        if (request.isForMainFrame && !done) {
          done = true
          NativeBridge.emit("loginError") {
            put("message", "Couldn't load the sign-in page: ${error.description}")
          }
          finish()
        }
      }
    }
    setContentView(web)
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        if (web.canGoBack()) web.goBack() else finish()
      }
    })
    web.loadUrl(if (addAccount) ADD_ACCOUNT_URL else LOGIN_URL)
  }

  private fun check(url: String) {
    if (done || android.net.Uri.parse(url).host != "music.youtube.com") return
    val cookie = CookieManager.getInstance().getCookie("https://music.youtube.com") ?: return
    if (!cookie.contains("SAPISID=")) return
    done = true
    CookieManager.getInstance().flush()
    NativeBridge.emit("loginCookie") { put("cookie", cookie) }
    finish()
  }

  override fun onDestroy() {
    if (!done && isFinishing) {
      NativeBridge.emit("loginError") { put("message", "Sign-in was cancelled") }
    }
    web.destroy()
    super.onDestroy()
  }
}
