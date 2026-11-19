package dev.rrgmc.ticktapdrum;

import android.app.NativeActivity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.util.Log;
import android.util.SparseArray;

/**
 * The app's activity: Android's NativeActivity, which runs libticktapdrum.so, plus the system
 * file picker (the Storage Access Framework).
 *
 * The picker returns its answer to the activity that started it, through onActivityResult.
 * NativeActivity does not pass that on to native code, so this class does: the Rust library
 * calls pickFile, and the chosen document's address goes back to it through onFilePicked.
 * Everything else, including opening the document, happens in Rust (apps/dm-android).
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

    /**
     * Hands a pick's answer to the Rust library: the chosen document's content:// address, or
     * null when the person backed out.
     */
    private static native void onFilePicked(long id, String uri);
}
