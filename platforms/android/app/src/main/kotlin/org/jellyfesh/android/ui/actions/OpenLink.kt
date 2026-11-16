package org.jellyfesh.android.ui.actions

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import org.jellyfesh.logic.sources.Links

/**
 * Open in {Service}: hands [link] to the system with `ACTION_VIEW`, so the service's own app opens
 * it when it handles such links, else the browser. Only `https` links the core sent are opened
 * ([Links.openable]). False when nothing could open it (`ActivityNotFoundException`, which apps
 * targeting Android 11 and later are told to expect rather than query for).
 */
fun openLinkIntent(context: Context, link: String): Boolean {
    val url = Links.openable(link) ?: return false
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).addCategory(Intent.CATEGORY_BROWSABLE)
    if (context !is Activity) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    return try {
        context.startActivity(intent)
        true
    } catch (e: ActivityNotFoundException) {
        false
    }
}
