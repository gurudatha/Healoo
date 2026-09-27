package com.healoo.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/** The full-file cache keeps the 15 most recently used files and at most 90 MB (AttachmentFiles.trim). */
class AttachmentCacheTest {
    @get:Rule val tmp = TemporaryFolder()

    /** Files named f0..f(n-1); f0 is the oldest. */
    private fun files(dir: File, n: Int, bytes: Int = 1): List<File> = (0 until n).map { i ->
        File(dir, "f$i").apply { writeBytes(ByteArray(bytes)); setLastModified(1_000_000L + i * 1000L) }
    }

    @Test fun keepsOnlyTheNewest15() {
        val dir = tmp.newFolder()
        files(dir, 20)
        AttachmentFiles.trim(dir)
        val left = dir.list()!!.toSet()
        assertEquals(AttachmentFiles.MAX_FILES, left.size)
        assertEquals((5 until 20).map { "f$it" }.toSet(), left)   // f0..f4 (oldest) made way
    }

    @Test fun evictsOldestOverTheByteLimit() {
        val dir = tmp.newFolder()
        files(dir, 6, bytes = 20 * 1024 * 1024)                    // 6 × 20 MB = 120 MB > 90 MB
        AttachmentFiles.trim(dir)
        assertEquals(setOf("f2", "f3", "f4", "f5"), dir.list()!!.toSet())   // 80 MB kept
    }

    @Test fun reusedFileCountsAsNew() {
        val dir = tmp.newFolder()
        val all = files(dir, 15)
        all[0].setLastModified(9_000_000L)                           // f0 was just opened again
        File(dir, "new").apply { writeBytes(ByteArray(1)); setLastModified(9_500_000L) }
        AttachmentFiles.trim(dir)
        val left = dir.list()!!.toSet()
        assertTrue("f0" in left && "new" in left)
        assertTrue("f1" !in left)                                     // now the oldest
    }

    @Test fun ignoresDownloadsInProgress() {
        val dir = tmp.newFolder()
        files(dir, 15)
        File(dir, "big.part").writeBytes(ByteArray(1))
        AttachmentFiles.trim(dir)
        assertTrue(File(dir, "big.part").exists())
        assertEquals(16, dir.list()!!.size)
    }
}
