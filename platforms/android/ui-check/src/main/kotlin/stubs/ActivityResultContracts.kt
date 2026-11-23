// Stand-ins for androidx.activity.result.contract (activity 1.13). Bodies never run.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.activity.result.contract

import android.content.Context
import android.content.Intent
import android.net.Uri

abstract class ActivityResultContract<I, O> {
    abstract fun createIntent(context: Context, input: I): Intent
    abstract fun parseResult(resultCode: Int, intent: Intent?): O
}

class ActivityResultContracts private constructor() {
    open class OpenDocumentTree : ActivityResultContract<Uri?, Uri?>() {
        override fun createIntent(context: Context, input: Uri?): Intent = TODO()
        override fun parseResult(resultCode: Int, intent: Intent?): Uri? = TODO()
    }

    open class OpenDocument : ActivityResultContract<Array<String>, Uri?>() {
        override fun createIntent(context: Context, input: Array<String>): Intent = TODO()
        override fun parseResult(resultCode: Int, intent: Intent?): Uri? = TODO()
    }

    open class OpenMultipleDocuments : ActivityResultContract<Array<String>, List<@JvmSuppressWildcards Uri>>() {
        override fun createIntent(context: Context, input: Array<String>): Intent = TODO()
        override fun parseResult(resultCode: Int, intent: Intent?): List<Uri> = TODO()
    }

    open class CreateDocument(mimeType: String) : ActivityResultContract<String, Uri?>() {
        override fun createIntent(context: Context, input: String): Intent = TODO()
        override fun parseResult(resultCode: Int, intent: Intent?): Uri? = TODO()
    }

    class RequestPermission : ActivityResultContract<String, Boolean>() {
        override fun createIntent(context: Context, input: String): Intent = TODO()
        override fun parseResult(resultCode: Int, intent: Intent?): Boolean = TODO()
    }
}
