#!/system/bin/sh
# Copyright 2023-2025, shadow3, shadow3aaa
#
# This file is part of fas-rs.
#
# fas-rs is free software: you can redistribute it and/or modify it under
# the terms of the GNU General Public License as published by the Free
# Software Foundation, either version 3 of the License, or (at your option)
# any later version.
#
# fas-rs is distributed in the hope that it will be useful, but WITHOUT ANY
# WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
# details.
#
# You should have received a copy of the GNU General Public License along
# with fas-rs. If not, see <https://www.gnu.org/licenses/>.

#do not enable unless i tell you to!
NO_COMPATIBILITY=0

SCENE_DIR=/data/data/com.omarea.vtools/files
DIR=/sdcard/Android/fas-rs
CONF=$DIR/games.toml
MERGE_FLAG=$DIR/.need_merge
LOCALE=$(getprop persist.sys.locale)
tuner_exist=`grep -E "SCENE" $SCENE_DIR/manifest.json`
tuner_unpatchable=`grep -E "Customized|Unofficial" $SCENE_DIR/manifest.json`
scene_config=`grep -E "LP|HP|EP" $SCENE_DIR/manifest.json`
mod_exist=`grep -E "fas_rs_mod" $SCENE_DIR/manifest.json`

local_print() {
	if [ $LOCALE = zh-CN ]; then
		ui_print "$1"
	else
		ui_print "$2"
	fi
}

local_echo() {
	if [ $LOCALE = zh-CN ]; then
		echo "$1"
	else
		echo "$2"
	fi
}

compatibility_check() {
if [ $ARCH != arm64 ]; then
	local_print "- 设备不支持, 非arm64设备" "- Only for arm64 device !"
	abort
elif [ $API -le 30 ]; then
	local_print "- 系统版本过低, 需要安卓12及以上的系统版本版本" "- Required A12+ !"
	abort
elif uname -r | awk -F. '{if ($1 < 5 || ($1 == 5 && $2 < 8)) exit 0; else exit 1}'; then
	local_print "- 内核版本过低，需要5.8或以上 !" "- The kernel version is too low. Requires 5.8+ !"
	abort
elif [ -z "$tuner_exist" ]; then
    local_print "- 检测不到SCENE调度。" "- Can't find SCENE tuner."
    abort
elif [ -n "$tuner_unpatchable" ]; then
    local_print "- 不支持修补此配置文件，请先切换到官方的任一调度配置(LP/HP/EP)。" "- This config is unpatchable, please switch to any official tuner config(LP/HP/EP) first !"
    abort
elif [ -n "$mod_exist" ]; then
    local_print "- 在SCENE性能调节页面点击*切换*，点击一次想使用的配置文件，再刷入此更新。" "- Click *switch* in SCENE's tuner, click the config you want to use, and flash this update again."
    abort
elif [ -z "$scene_config" ]; then
    local_print "- 不支持的SCENE配置文件。" "- Unsupported SCENE config."
    abort
fi
}

if [ "$NO_COMPATIBILITY" = "1" ]; then
    local_print "- 已跳过兼容性检查。" "- Compatibility check skipped."
    echo "$NO_COMPATIBILITY"
elif [ "$NO_COMPATIBILITY" = "0" ]; then
    compatibility_check
fi

if [ -f $CONF ]; then
	touch $MERGE_FLAG
else
	mkdir -p $DIR
	cp $MODPATH/games.toml $CONF
fi

cp -f $MODPATH/README_CN.md $DIR/doc_cn.md
cp -f $MODPATH/README_EN.md $DIR/doc_en.md

set_perm_recursive $MODPATH 0 0 0755 0644
set_perm $MODPATH/fas-rs 0 0 0755
set_perm $MODPATH/vtools/jq 0 0 0755

local_print "配置文件夹：/sdcard/Android/fas-rs" "Configuration folder: /sdcard/Android/fas-rs"
local_echo "updateJson=https://github.com/shadow3aaa/fas-rs/raw/master/update/update.json" "updateJson=https://github.com/shadow3aaa/fas-rs/raw/master/update/update_en.json" >>$MODPATH/module.prop
