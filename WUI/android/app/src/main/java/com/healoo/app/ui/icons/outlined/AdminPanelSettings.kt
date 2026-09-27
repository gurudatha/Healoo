/*
 * Copyright 2025 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * Copied from androidx.compose.material:material-icons-extended 1.7.8 so the app ships only the
 * icons it uses instead of the whole library (about 34 MB of code before shrinking).
 */

package com.healoo.app.ui.icons.outlined

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.materialIcon
import androidx.compose.material.icons.materialPath
import androidx.compose.ui.graphics.PathFillType.Companion.EvenOdd
import androidx.compose.ui.graphics.vector.ImageVector

internal val Icons.Outlined.AdminPanelSettings: ImageVector
    get() {
        if (_adminPanelSettings != null) {
            return _adminPanelSettings!!
        }
        _adminPanelSettings = materialIcon(name = "Outlined.AdminPanelSettings") {
            materialPath(pathFillType = EvenOdd) {
                moveTo(17.0f, 15.5f)
                moveToRelative(-1.12f, 0.0f)
                arcToRelative(1.12f, 1.12f, 0.0f, true, true, 2.24f, 0.0f)
                arcToRelative(1.12f, 1.12f, 0.0f, true, true, -2.24f, 0.0f)
            }
            materialPath(pathFillType = EvenOdd) {
                moveTo(17.0f, 17.5f)
                curveToRelative(-0.73f, 0.0f, -2.19f, 0.36f, -2.24f, 1.08f)
                curveToRelative(0.5f, 0.71f, 1.32f, 1.17f, 2.24f, 1.17f)
                reflectiveCurveToRelative(1.74f, -0.46f, 2.24f, -1.17f)
                curveTo(19.19f, 17.86f, 17.73f, 17.5f, 17.0f, 17.5f)
                close()
            }
            materialPath(pathFillType = EvenOdd) {
                moveTo(18.0f, 11.09f)
                verticalLineTo(6.27f)
                lineTo(10.5f, 3.0f)
                lineTo(3.0f, 6.27f)
                verticalLineToRelative(4.91f)
                curveToRelative(0.0f, 4.54f, 3.2f, 8.79f, 7.5f, 9.82f)
                curveToRelative(0.55f, -0.13f, 1.08f, -0.32f, 1.6f, -0.55f)
                curveTo(13.18f, 21.99f, 14.97f, 23.0f, 17.0f, 23.0f)
                curveToRelative(3.31f, 0.0f, 6.0f, -2.69f, 6.0f, -6.0f)
                curveTo(23.0f, 14.03f, 20.84f, 11.57f, 18.0f, 11.09f)
                close()
                moveTo(11.0f, 17.0f)
                curveToRelative(0.0f, 0.56f, 0.08f, 1.11f, 0.23f, 1.62f)
                curveToRelative(-0.24f, 0.11f, -0.48f, 0.22f, -0.73f, 0.3f)
                curveToRelative(-3.17f, -1.0f, -5.5f, -4.24f, -5.5f, -7.74f)
                verticalLineToRelative(-3.6f)
                lineToRelative(5.5f, -2.4f)
                lineToRelative(5.5f, 2.4f)
                verticalLineToRelative(3.51f)
                curveTo(13.16f, 11.57f, 11.0f, 14.03f, 11.0f, 17.0f)
                close()
                moveTo(17.0f, 21.0f)
                curveToRelative(-2.21f, 0.0f, -4.0f, -1.79f, -4.0f, -4.0f)
                curveToRelative(0.0f, -2.21f, 1.79f, -4.0f, 4.0f, -4.0f)
                reflectiveCurveToRelative(4.0f, 1.79f, 4.0f, 4.0f)
                curveTo(21.0f, 19.21f, 19.21f, 21.0f, 17.0f, 21.0f)
                close()
            }
        }
        return _adminPanelSettings!!
    }

private var _adminPanelSettings: ImageVector? = null
