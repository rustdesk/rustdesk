package moe.shizuku.api

import android.os.IBinder
import android.os.Parcel
import android.os.Parcelable

/**
 * Wire-format class of the Shizuku v3 binder-transfer protocol.
 *
 * The Shizuku server delivers its service binder to a client app by calling
 * the app's ContentProvider with method "sendBinder" and a Bundle that holds
 * the binder wrapped in a parcelable of this fixed class name
 * (RikkaApps/Shizuku v3.6.1, ShizukuService.sendBinderToUserApp). The parcel
 * layout is a single strong binder reference.
 *
 * Implemented here instead of vendoring the upstream client library
 * (moe.shizuku.privilege:api), which is no longer published on any maven
 * repository. The class name and layout are dictated by the protocol; the
 * implementation is ours.
 */
class BinderContainer(val binder: IBinder?) : Parcelable {
    constructor(parcel: Parcel) : this(parcel.readStrongBinder())

    override fun writeToParcel(dest: Parcel, flags: Int) {
        dest.writeStrongBinder(binder)
    }

    override fun describeContents(): Int = 0

    companion object CREATOR : Parcelable.Creator<BinderContainer> {
        override fun createFromParcel(source: Parcel): BinderContainer = BinderContainer(source)

        override fun newArray(size: Int): Array<BinderContainer?> = arrayOfNulls(size)
    }
}
