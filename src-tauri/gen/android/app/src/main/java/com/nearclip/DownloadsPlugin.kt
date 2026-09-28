package com.nearclip

import android.app.Activity
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class SaveArgs {
  lateinit var path: String
  lateinit var name: String
  var mime: String = ""
}

@InvokeArg
class UriArgs {
  lateinit var uri: String
  var mime: String = ""
}

/**
 * Puts a received file where the user expects to find it: the phone's Downloads
 * folder, visible in the Files app and in every app's file picker, then opens or
 * shares it from there.
 *
 * An app cannot simply write to that folder. Since Android 10 the only supported
 * way in is MediaStore, which owns it and hands back a content URI; a file left
 * in the app's own directory is hidden from the Files app and looks lost.
 *
 * Called from `src-tauri/src/downloads.rs`. Exceptions thrown here are turned
 * into a rejected call by the Tauri plugin framework.
 */
@TauriPlugin
class DownloadsPlugin(private val activity: Activity) : Plugin(activity) {
  /**
   * Copies a received file into Downloads/NearClip and answers with its URI and
   * the name it ended up with, which is not the requested one when MediaStore
   * has to avoid a collision.
   */
  @Command
  fun save(invoke: Invoke) {
    val args = invoke.parseArgs(SaveArgs::class.java)
    // Commands run on the main thread. Copying a file of up to 100 MB there
    // freezes the interface for seconds and invites an ANR, so the copy goes to
    // a background thread and answers from it, which the bridge allows.
    Thread {
        try {
          val source = File(args.path)
          val uri =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
              saveWithMediaStore(source, args.name, args.mime)
            } else {
              savePublicDirectory(source, args.name)
            }
          invoke.resolve(
            JSObject().put("uri", uri.toString()).put("name", savedName(uri, args.name))
          )
        } catch (e: Exception) {
          invoke.reject(e.message ?: e.toString())
        }
      }
      .start()
  }

  /**
   * Opens the file in whichever app handles its type.
   *
   * This does what `tauri-plugin-opener` does, plus the one thing it leaves out:
   * a content URI is readable by another app only if the intent carries a read
   * grant. Without it the viewer opens on an empty file.
   */
  @Command
  fun open(invoke: Invoke) {
    val args = invoke.parseArgs(UriArgs::class.java)
    val uri = Uri.parse(args.uri)
    activity.startActivity(
      Intent(Intent.ACTION_VIEW).apply {
        setDataAndType(uri, args.mime.ifEmpty { "*/*" })
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
      }
    )
    invoke.resolve()
  }

  @Command
  fun share(invoke: Invoke) {
    val args = invoke.parseArgs(UriArgs::class.java)
    val uri = Uri.parse(args.uri)
    val mime = args.mime.ifEmpty { "*/*" }
    val send =
      Intent(Intent.ACTION_SEND).apply {
        type = mime
        putExtra(Intent.EXTRA_STREAM, uri)
        // The chooser shows a preview only when the URI is also in the clip data.
        clipData = android.content.ClipData.newUri(activity.contentResolver, "", uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      }
    activity.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    invoke.resolve()
  }

  private fun saveWithMediaStore(source: File, name: String, mime: String): Uri {
    val resolver = activity.contentResolver
    val values =
      ContentValues().apply {
        put(MediaStore.Downloads.DISPLAY_NAME, name)
        if (mime.isNotEmpty()) put(MediaStore.Downloads.MIME_TYPE, mime)
        put(MediaStore.Downloads.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/$FOLDER")
        // Hidden from other apps until the bytes are all there.
        put(MediaStore.Downloads.IS_PENDING, 1)
      }
    val uri =
      resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
        ?: throw IllegalStateException("could not create $name in Downloads")
    try {
      // A megabyte at a time: the stream is unbuffered, and on Android 11+ every
      // write crosses into the media provider, so 8 KiB chunks cost ten times
      // the transfer in round trips.
      resolver.openOutputStream(uri)?.use { out ->
        source.inputStream().use { it.copyTo(out, COPY_BUFFER) }
      } ?: throw IllegalStateException("could not write $name to Downloads")
    } catch (e: Exception) {
      resolver.delete(uri, null, null)
      throw e
    }
    resolver.update(uri, ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) }, null, null)
    return uri
  }

  /** Android 9 and older: the public folder is writable directly. */
  private fun savePublicDirectory(source: File, name: String): Uri {
    val dir =
      File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS), FOLDER)
    dir.mkdirs()
    val target = File(dir, name)
    source.inputStream().use { input ->
      target.outputStream().use { input.copyTo(it, COPY_BUFFER) }
    }
    return FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", target)
  }

  /** What the file is actually called now, which MediaStore may have changed. */
  private fun savedName(uri: Uri, requested: String): String {
    if (uri.scheme != "content") return uri.lastPathSegment ?: requested
    return activity.contentResolver
      .query(uri, arrayOf(MediaStore.Downloads.DISPLAY_NAME), null, null, null)
      ?.use { if (it.moveToFirst()) it.getString(0) else null } ?: requested
  }

  private companion object {
    const val FOLDER = "NearClip"
    const val COPY_BUFFER = 1 shl 20
  }
}
