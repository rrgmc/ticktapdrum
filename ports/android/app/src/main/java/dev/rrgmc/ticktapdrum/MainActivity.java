package dev.rrgmc.ticktapdrum;

import android.app.NativeActivity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.media.AudioManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
import android.util.SparseArray;
import android.view.WindowManager;

/**
 * The app's activity: Android's NativeActivity, which runs libticktapdrum.so, plus the system
 * file picker (the Storage Access Framework).
 *
 * The picker returns its answer to the activity that started it, through onActivityResult.
 * NativeActivity does not pass that on to native code, so this class does: the Rust library
 * calls pickFile, and the chosen document's address goes back to it through onFilePicked.
 * Everything else, including opening the document, happens in Rust (apps/dm-android).
 *
 * It also passes on what other apps share with this one, or open in it: a link, or a MIDI or zip
 * file. NativeActivity does not pass intents to native code either. The Rust library receives
 * them through onShared (see apps/dm-android/src/share.rs).
 */
public class MainActivity extends NativeActivity {
    static {
        // NativeActivity opens the library from native code, which does not let Java find the
        // library's native methods. Loading it here as well registers it for that. Both loads
        // open the same file, so the library is loaded once.
        System.loadLibrary("ticktapdrum");
    }

    private static final String TAG = "ticktapdrum";

    /** The picks in progress, by request code. Used on the UI thread only. */
    private final SparseArray<Long> pending = new SparseArray<>();
    private int nextCode = 1;

    /**
     * Keeps the screen on, or lets it turn off again, through the window's FLAG_KEEP_SCREEN_ON.
     * Callable from any thread: the change is made on the UI thread, where Android needs it.
     *
     * The Rust library calls this instead of android-activity's set_window_flags, which takes
     * a write lock that the app's own frame already holds for reading, and so never returns.
     */
    public void keepScreenOn(final boolean on) {
        runOnUiThread(() -> {
            if (on) {
                getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            } else {
                getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            }
        });
    }

    /**
     * Shows the system picker: "Save as" when create is true, otherwise "Open". The answer goes
     * to onFilePicked with the same id. Callable from any thread.
     *
     * @param id   identifies the answer; passed back unchanged
     * @param mime the MIME type to offer, or to give the new file
     * @param name the suggested file name for a new file
     */
    public void pickFile(final long id, final boolean create, final String mime, final String name) {
        runOnUiThread(() -> startPicker(id, create, mime, name));
    }

    private void startPicker(long id, boolean create, String mime, String name) {
        Intent intent = new Intent(create ? Intent.ACTION_CREATE_DOCUMENT : Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType(mime);
        if (create) {
            intent.putExtra(Intent.EXTRA_TITLE, name);
        }
        // Request codes must fit in 16 bits.
        int code = nextCode;
        nextCode = nextCode % 0xffff + 1;
        pending.put(code, id);
        try {
            startActivityForResult(intent, code);
        } catch (ActivityNotFoundException e) {
            // No app on the device offers documents. Answer at once, as if the person backed out.
            pending.remove(code);
            Log.w(TAG, "no file picker on this device", e);
            onFilePicked(id, null);
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        Long id = pending.get(requestCode);
        if (id == null) {
            return;
        }
        pending.remove(requestCode);
        Uri uri = resultCode == RESULT_OK && data != null ? data.getData() : null;
        onFilePicked(id, uri == null ? null : uri.toString());
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // The volume keys set the media volume the drums play at, even while the app is silent,
        // instead of the ringer.
        setVolumeControlStream(AudioManager.STREAM_MUSIC);
        // A saved state means Android restarted the activity, and its intent was handled before.
        if (savedInstanceState == null) {
            handleShare(getIntent());
        }
    }

    @Override
    protected void onNewIntent(Intent intent) {
        // The activity is singleTask, so a share into the running app arrives here.
        super.onNewIntent(intent);
        setIntent(intent);
        handleShare(intent);
    }

    /**
     * Hands a share or an "Open with" to the Rust library: ACTION_SEND with text (a link) or a
     * file, or ACTION_VIEW with a file. Other intents, such as the launcher's, are ignored.
     */
    private void handleShare(Intent intent) {
        if (intent == null) {
            return;
        }
        // Opening the app again from the recent apps list repeats the intent that started it.
        if ((intent.getFlags() & Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY) != 0) {
            return;
        }
        String action = intent.getAction();
        String text = null;
        Uri uri = null;
        if (Intent.ACTION_SEND.equals(action)) {
            uri = streamOf(intent);
            CharSequence shared = intent.getCharSequenceExtra(Intent.EXTRA_TEXT);
            // Some apps send text and a file together. For a text type the text is used, and
            // otherwise the file.
            String type = intent.getType();
            boolean isText = type != null && type.startsWith("text/");
            if (shared != null && (uri == null || isText)) {
                text = shared.toString();
                uri = null;
            }
        } else if (Intent.ACTION_VIEW.equals(action)) {
            uri = intent.getData();
        } else {
            return;
        }
        if (text == null && uri == null) {
            Log.w(TAG, "a share with nothing in it: " + intent);
            return;
        }
        onShared(text, uri == null ? null : uri.toString());
        // Handled: a later look at the intent, after a restart, finds nothing to do.
        intent.setAction(null);
    }

    /** The file a SEND intent carries, its EXTRA_STREAM, or null. */
    @SuppressWarnings("deprecation")
    private static Uri streamOf(Intent intent) {
        // Android 13 added a typed form and deprecated the old one, which still works. The typed
        // form has a known bug on Android 13 itself, so it is used from Android 14 on.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            return intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri.class);
        }
        return intent.getParcelableExtra(Intent.EXTRA_STREAM);
    }

    /**
     * Hands a pick's answer to the Rust library: the chosen document's content:// address, or
     * null when the person backed out.
     */
    private static native void onFilePicked(long id, String uri);

    /**
     * Hands a share to the Rust library: text such as a link, or the address of a file
     * (content:// or file://). One of the two is null. The library reads the file while the
     * activity holds the permission to read it that the share granted.
     */
    private static native void onShared(String text, String uri);
}
