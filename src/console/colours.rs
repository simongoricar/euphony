#![allow(unused)]

// This module contains the 256 colours available in a terminal supporting 8-bit colour.
//
// Source: https://github.com/jonasjacek/colors and https://www.ditig.com/256-colors-cheat-sheet


use ratatui::style::{Color, Style};

/// Colour 0 is Black (#000000)
pub const X000_BLACK: Style = Style::new().fg(Color::Indexed(0));

/// Colour 1 is Maroon (#800000)
pub const X001_MAROON: Style = Style::new().fg(Color::Indexed(1));

/// Colour 2 is Green (#008000)
pub const X002_GREEN: Style = Style::new().fg(Color::Indexed(2));

/// Colour 3 is Olive (#808000)
pub const X003_OLIVE: Style = Style::new().fg(Color::Indexed(3));

/// Colour 4 is Navy (#000080)
pub const X004_NAVY: Style = Style::new().fg(Color::Indexed(4));

/// Colour 5 is Purple (#800080)
pub const X005_PURPLE: Style = Style::new().fg(Color::Indexed(5));

/// Colour 6 is Teal (#008080)
pub const X006_TEAL: Style = Style::new().fg(Color::Indexed(6));

/// Colour 7 is Silver (#c0c0c0)
pub const X007_SILVER: Style = Style::new().fg(Color::Indexed(7));

/// Colour 8 is Grey (#808080)
pub const X008_GREY: Style = Style::new().fg(Color::Indexed(8));

/// Colour 9 is Red (#ff0000)
pub const X009_RED: Style = Style::new().fg(Color::Indexed(9));

/// Colour 10 is Lime (#00ff00)
pub const X010_LIME: Style = Style::new().fg(Color::Indexed(10));

/// Colour 11 is Yellow (#ffff00)
pub const X011_YELLOW: Style = Style::new().fg(Color::Indexed(11));

/// Colour 12 is Blue (#0000ff)
pub const X012_BLUE: Style = Style::new().fg(Color::Indexed(12));

/// Colour 13 is Fuchsia (#ff00ff)
pub const X013_FUCHSIA: Style = Style::new().fg(Color::Indexed(13));

/// Colour 14 is Aqua (#00ffff)
pub const X014_AQUA: Style = Style::new().fg(Color::Indexed(14));

/// Colour 15 is White (#ffffff)
pub const X015_WHITE: Style = Style::new().fg(Color::Indexed(15));

/// Colour 16 is Grey0 (#000000)
pub const X016_GREY0: Style = Style::new().fg(Color::Indexed(16));

/// Colour 17 is NavyBlue (#00005f)
pub const X017_NAVY_BLUE: Style = Style::new().fg(Color::Indexed(17));

/// Colour 18 is DarkBlue (#000087)
pub const X018_DARK_BLUE: Style = Style::new().fg(Color::Indexed(18));

/// Colour 19 is Blue3 (#0000af)
pub const X019_BLUE3: Style = Style::new().fg(Color::Indexed(19));

/// Colour 20 is Blue3 (#0000d7)
pub const X020_BLUE3: Style = Style::new().fg(Color::Indexed(20));

/// Colour 21 is Blue1 (#0000ff)
pub const X021_BLUE1: Style = Style::new().fg(Color::Indexed(21));

/// Colour 22 is DarkGreen (#005f00)
pub const X022_DARK_GREEN: Style = Style::new().fg(Color::Indexed(22));

/// Colour 23 is DeepSkyBlue4 (#005f5f)
pub const X023_DEEP_SKY_BLUE4: Style = Style::new().fg(Color::Indexed(23));

/// Colour 24 is DeepSkyBlue4 (#005f87)
pub const X024_DEEP_SKY_BLUE4: Style = Style::new().fg(Color::Indexed(24));

/// Colour 25 is DeepSkyBlue4 (#005faf)
pub const X025_DEEP_SKY_BLUE4: Style = Style::new().fg(Color::Indexed(25));

/// Colour 26 is DodgerBlue3 (#005fd7)
pub const X026_DODGER_BLUE3: Style = Style::new().fg(Color::Indexed(26));

/// Colour 27 is DodgerBlue2 (#005fff)
pub const X027_DODGER_BLUE2: Style = Style::new().fg(Color::Indexed(27));

/// Colour 28 is Green4 (#008700)
pub const X028_GREEN4: Style = Style::new().fg(Color::Indexed(28));

/// Colour 29 is SpringGreen4 (#00875f)
pub const X029_SPRING_GREEN4: Style = Style::new().fg(Color::Indexed(29));

/// Colour 30 is Turquoise4 (#008787)
pub const X030_TURQUOISE4: Style = Style::new().fg(Color::Indexed(30));

/// Colour 31 is DeepSkyBlue3 (#0087af)
pub const X031_DEEP_SKY_BLUE3: Style = Style::new().fg(Color::Indexed(31));

/// Colour 32 is DeepSkyBlue3 (#0087d7)
pub const X032_DEEP_SKY_BLUE3: Style = Style::new().fg(Color::Indexed(32));

/// Colour 33 is DodgerBlue1 (#0087ff)
pub const X033_DODGER_BLUE1: Style = Style::new().fg(Color::Indexed(33));

/// Colour 34 is Green3 (#00af00)
pub const X034_GREEN3: Style = Style::new().fg(Color::Indexed(34));

/// Colour 35 is SpringGreen3 (#00af5f)
pub const X035_SPRING_GREEN3: Style = Style::new().fg(Color::Indexed(35));

/// Colour 36 is DarkCyan (#00af87)
pub const X036_DARK_CYAN: Style = Style::new().fg(Color::Indexed(36));

/// Colour 37 is LightSeaGreen (#00afaf)
pub const X037_LIGHT_SEA_GREEN: Style = Style::new().fg(Color::Indexed(37));

/// Colour 38 is DeepSkyBlue2 (#00afd7)
pub const X038_DEEP_SKY_BLUE2: Style = Style::new().fg(Color::Indexed(38));

/// Colour 39 is DeepSkyBlue1 (#00afff)
pub const X039_DEEP_SKY_BLUE1: Style = Style::new().fg(Color::Indexed(39));

/// Colour 40 is Green3 (#00d700)
pub const X040_GREEN3: Style = Style::new().fg(Color::Indexed(40));

/// Colour 41 is SpringGreen3 (#00d75f)
pub const X041_SPRING_GREEN3: Style = Style::new().fg(Color::Indexed(41));

/// Colour 42 is SpringGreen2 (#00d787)
pub const X042_SPRING_GREEN2: Style = Style::new().fg(Color::Indexed(42));

/// Colour 43 is Cyan3 (#00d7af)
pub const X043_CYAN3: Style = Style::new().fg(Color::Indexed(43));

/// Colour 44 is DarkTurquoise (#00d7d7)
pub const X044_DARK_TURQUOISE: Style = Style::new().fg(Color::Indexed(44));

/// Colour 45 is Turquoise2 (#00d7ff)
pub const X045_TURQUOISE2: Style = Style::new().fg(Color::Indexed(45));

/// Colour 46 is Green1 (#00ff00)
pub const X046_GREEN1: Style = Style::new().fg(Color::Indexed(46));

/// Colour 47 is SpringGreen2 (#00ff5f)
pub const X047_SPRING_GREEN2: Style = Style::new().fg(Color::Indexed(47));

/// Colour 48 is SpringGreen1 (#00ff87)
pub const X048_SPRING_GREEN1: Style = Style::new().fg(Color::Indexed(48));

/// Colour 49 is MediumSpringGreen (#00ffaf)
pub const X049_MEDIUM_SPRING_GREEN: Style = Style::new().fg(Color::Indexed(49));

/// Colour 50 is Cyan2 (#00ffd7)
pub const X050_CYAN2: Style = Style::new().fg(Color::Indexed(50));

/// Colour 51 is Cyan1 (#00ffff)
pub const X051_CYAN1: Style = Style::new().fg(Color::Indexed(51));

/// Colour 52 is DarkRed (#5f0000)
pub const X052_DARK_RED: Style = Style::new().fg(Color::Indexed(52));

/// Colour 53 is DeepPink4 (#5f005f)
pub const X053_DEEP_PINK4: Style = Style::new().fg(Color::Indexed(53));

/// Colour 54 is Purple4 (#5f0087)
pub const X054_PURPLE4: Style = Style::new().fg(Color::Indexed(54));

/// Colour 55 is Purple4 (#5f00af)
pub const X055_PURPLE4: Style = Style::new().fg(Color::Indexed(55));

/// Colour 56 is Purple3 (#5f00d7)
pub const X056_PURPLE3: Style = Style::new().fg(Color::Indexed(56));

/// Colour 57 is BlueViolet (#5f00ff)
pub const X057_BLUE_VIOLET: Style = Style::new().fg(Color::Indexed(57));

/// Colour 58 is Orange4 (#5f5f00)
pub const X058_ORANGE4: Style = Style::new().fg(Color::Indexed(58));

/// Colour 59 is Grey37 (#5f5f5f)
pub const X059_GREY37: Style = Style::new().fg(Color::Indexed(59));

/// Colour 60 is MediumPurple4 (#5f5f87)
pub const X060_MEDIUM_PURPLE4: Style = Style::new().fg(Color::Indexed(60));

/// Colour 61 is SlateBlue3 (#5f5faf)
pub const X061_SLATE_BLUE3: Style = Style::new().fg(Color::Indexed(61));

/// Colour 62 is SlateBlue3 (#5f5fd7)
pub const X062_SLATE_BLUE3: Style = Style::new().fg(Color::Indexed(62));

/// Colour 63 is RoyalBlue1 (#5f5fff)
pub const X063_ROYAL_BLUE1: Style = Style::new().fg(Color::Indexed(63));

/// Colour 64 is Chartreuse4 (#5f8700)
pub const X064_CHARTREUSE4: Style = Style::new().fg(Color::Indexed(64));

/// Colour 65 is DarkSeaGreen4 (#5f875f)
pub const X065_DARK_SEA_GREEN4: Style = Style::new().fg(Color::Indexed(65));

/// Colour 66 is PaleTurquoise4 (#5f8787)
pub const X066_PALE_TURQUOISE4: Style = Style::new().fg(Color::Indexed(66));

/// Colour 67 is SteelBlue (#5f87af)
pub const X067_STEEL_BLUE: Style = Style::new().fg(Color::Indexed(67));

/// Colour 68 is SteelBlue3 (#5f87d7)
pub const X068_STEEL_BLUE3: Style = Style::new().fg(Color::Indexed(68));

/// Colour 69 is CornflowerBlue (#5f87ff)
pub const X069_CORNFLOWER_BLUE: Style = Style::new().fg(Color::Indexed(69));

/// Colour 70 is Chartreuse3 (#5faf00)
pub const X070_CHARTREUSE3: Style = Style::new().fg(Color::Indexed(70));

/// Colour 71 is DarkSeaGreen4 (#5faf5f)
pub const X071_DARK_SEA_GREEN4: Style = Style::new().fg(Color::Indexed(71));

/// Colour 72 is CadetBlue (#5faf87)
pub const X072_CADET_BLUE: Style = Style::new().fg(Color::Indexed(72));

/// Colour 73 is CadetBlue (#5fafaf)
pub const X073_CADET_BLUE: Style = Style::new().fg(Color::Indexed(73));

/// Colour 74 is SkyBlue3 (#5fafd7)
pub const X074_SKY_BLUE3: Style = Style::new().fg(Color::Indexed(74));

/// Colour 75 is SteelBlue1 (#5fafff)
pub const X075_STEEL_BLUE1: Style = Style::new().fg(Color::Indexed(75));

/// Colour 76 is Chartreuse3 (#5fd700)
pub const X076_CHARTREUSE3: Style = Style::new().fg(Color::Indexed(76));

/// Colour 77 is PaleGreen3 (#5fd75f)
pub const X077_PALE_GREEN3: Style = Style::new().fg(Color::Indexed(77));

/// Colour 78 is SeaGreen3 (#5fd787)
pub const X078_SEA_GREEN3: Style = Style::new().fg(Color::Indexed(78));

/// Colour 79 is Aquamarine3 (#5fd7af)
pub const X079_AQUAMARINE3: Style = Style::new().fg(Color::Indexed(79));

/// Colour 80 is MediumTurquoise (#5fd7d7)
pub const X080_MEDIUM_TURQUOISE: Style = Style::new().fg(Color::Indexed(80));

/// Colour 81 is SteelBlue1 (#5fd7ff)
pub const X081_STEEL_BLUE1: Style = Style::new().fg(Color::Indexed(81));

/// Colour 82 is Chartreuse2 (#5fff00)
pub const X082_CHARTREUSE2: Style = Style::new().fg(Color::Indexed(82));

/// Colour 83 is SeaGreen2 (#5fff5f)
pub const X083_SEA_GREEN2: Style = Style::new().fg(Color::Indexed(83));

/// Colour 84 is SeaGreen1 (#5fff87)
pub const X084_SEA_GREEN1: Style = Style::new().fg(Color::Indexed(84));

/// Colour 85 is SeaGreen1 (#5fffaf)
pub const X085_SEA_GREEN1: Style = Style::new().fg(Color::Indexed(85));

/// Colour 86 is Aquamarine1 (#5fffd7)
pub const X086_AQUAMARINE1: Style = Style::new().fg(Color::Indexed(86));

/// Colour 87 is DarkSlateGray2 (#5fffff)
pub const X087_DARK_SLATE_GRAY2: Style = Style::new().fg(Color::Indexed(87));

/// Colour 88 is DarkRed (#870000)
pub const X088_DARK_RED: Style = Style::new().fg(Color::Indexed(88));

/// Colour 89 is DeepPink4 (#87005f)
pub const X089_DEEP_PINK4: Style = Style::new().fg(Color::Indexed(89));

/// Colour 90 is DarkMagenta (#870087)
pub const X090_DARK_MAGENTA: Style = Style::new().fg(Color::Indexed(90));

/// Colour 91 is DarkMagenta (#8700af)
pub const X091_DARK_MAGENTA: Style = Style::new().fg(Color::Indexed(91));

/// Colour 92 is DarkViolet (#8700d7)
pub const X092_DARK_VIOLET: Style = Style::new().fg(Color::Indexed(92));

/// Colour 93 is Purple (#8700ff)
pub const X093_PURPLE: Style = Style::new().fg(Color::Indexed(93));

/// Colour 94 is Orange4 (#875f00)
pub const X094_ORANGE4: Style = Style::new().fg(Color::Indexed(94));

/// Colour 95 is LightPink4 (#875f5f)
pub const X095_LIGHT_PINK4: Style = Style::new().fg(Color::Indexed(95));

/// Colour 96 is Plum4 (#875f87)
pub const X096_PLUM4: Style = Style::new().fg(Color::Indexed(96));

/// Colour 97 is MediumPurple3 (#875faf)
pub const X097_MEDIUM_PURPLE3: Style = Style::new().fg(Color::Indexed(97));

/// Colour 98 is MediumPurple3 (#875fd7)
pub const X098_MEDIUM_PURPLE3: Style = Style::new().fg(Color::Indexed(98));

/// Colour 99 is SlateBlue1 (#875fff)
pub const X099_SLATE_BLUE1: Style = Style::new().fg(Color::Indexed(99));

/// Colour 100 is Yellow4 (#878700)
pub const X100_YELLOW4: Style = Style::new().fg(Color::Indexed(100));

/// Colour 101 is Wheat4 (#87875f)
pub const X101_WHEAT4: Style = Style::new().fg(Color::Indexed(101));

/// Colour 102 is Grey53 (#878787)
pub const X102_GREY53: Style = Style::new().fg(Color::Indexed(102));

/// Colour 103 is LightSlateGrey (#8787af)
pub const X103_LIGHT_SLATE_GREY: Style = Style::new().fg(Color::Indexed(103));

/// Colour 104 is MediumPurple (#8787d7)
pub const X104_MEDIUM_PURPLE: Style = Style::new().fg(Color::Indexed(104));

/// Colour 105 is LightSlateBlue (#8787ff)
pub const X105_LIGHT_SLATE_BLUE: Style = Style::new().fg(Color::Indexed(105));

/// Colour 106 is Yellow4 (#87af00)
pub const X106_YELLOW4: Style = Style::new().fg(Color::Indexed(106));

/// Colour 107 is DarkOliveGreen3 (#87af5f)
pub const X107_DARK_OLIVE_GREEN3: Style = Style::new().fg(Color::Indexed(107));

/// Colour 108 is DarkSeaGreen (#87af87)
pub const X108_DARK_SEA_GREEN: Style = Style::new().fg(Color::Indexed(108));

/// Colour 109 is LightSkyBlue3 (#87afaf)
pub const X109_LIGHT_SKY_BLUE3: Style = Style::new().fg(Color::Indexed(109));

/// Colour 110 is LightSkyBlue3 (#87afd7)
pub const X110_LIGHT_SKY_BLUE3: Style = Style::new().fg(Color::Indexed(110));

/// Colour 111 is SkyBlue2 (#87afff)
pub const X111_SKY_BLUE2: Style = Style::new().fg(Color::Indexed(111));

/// Colour 112 is Chartreuse2 (#87d700)
pub const X112_CHARTREUSE2: Style = Style::new().fg(Color::Indexed(112));

/// Colour 113 is DarkOliveGreen3 (#87d75f)
pub const X113_DARK_OLIVE_GREEN3: Style = Style::new().fg(Color::Indexed(113));

/// Colour 114 is PaleGreen3 (#87d787)
pub const X114_PALE_GREEN3: Style = Style::new().fg(Color::Indexed(114));

/// Colour 115 is DarkSeaGreen3 (#87d7af)
pub const X115_DARK_SEA_GREEN3: Style = Style::new().fg(Color::Indexed(115));

/// Colour 116 is DarkSlateGray3 (#87d7d7)
pub const X116_DARK_SLATE_GRAY3: Style = Style::new().fg(Color::Indexed(116));

/// Colour 117 is SkyBlue1 (#87d7ff)
pub const X117_SKY_BLUE1: Style = Style::new().fg(Color::Indexed(117));

/// Colour 118 is Chartreuse1 (#87ff00)
pub const X118_CHARTREUSE1: Style = Style::new().fg(Color::Indexed(118));

/// Colour 119 is LightGreen (#87ff5f)
pub const X119_LIGHT_GREEN: Style = Style::new().fg(Color::Indexed(119));

/// Colour 120 is LightGreen (#87ff87)
pub const X120_LIGHT_GREEN: Style = Style::new().fg(Color::Indexed(120));

/// Colour 121 is PaleGreen1 (#87ffaf)
pub const X121_PALE_GREEN1: Style = Style::new().fg(Color::Indexed(121));

/// Colour 122 is Aquamarine1 (#87ffd7)
pub const X122_AQUAMARINE1: Style = Style::new().fg(Color::Indexed(122));

/// Colour 123 is DarkSlateGray1 (#87ffff)
pub const X123_DARK_SLATE_GRAY1: Style = Style::new().fg(Color::Indexed(123));

/// Colour 124 is Red3 (#af0000)
pub const X124_RED3: Style = Style::new().fg(Color::Indexed(124));

/// Colour 125 is DeepPink4 (#af005f)
pub const X125_DEEP_PINK4: Style = Style::new().fg(Color::Indexed(125));

/// Colour 126 is MediumVioletRed (#af0087)
pub const X126_MEDIUM_VIOLET_RED: Style = Style::new().fg(Color::Indexed(126));

/// Colour 127 is Magenta3 (#af00af)
pub const X127_MAGENTA3: Style = Style::new().fg(Color::Indexed(127));

/// Colour 128 is DarkViolet (#af00d7)
pub const X128_DARK_VIOLET: Style = Style::new().fg(Color::Indexed(128));

/// Colour 129 is Purple (#af00ff)
pub const X129_PURPLE: Style = Style::new().fg(Color::Indexed(129));

/// Colour 130 is DarkOrange3 (#af5f00)
pub const X130_DARK_ORANGE3: Style = Style::new().fg(Color::Indexed(130));

/// Colour 131 is IndianRed (#af5f5f)
pub const X131_INDIAN_RED: Style = Style::new().fg(Color::Indexed(131));

/// Colour 132 is HotPink3 (#af5f87)
pub const X132_HOT_PINK3: Style = Style::new().fg(Color::Indexed(132));

/// Colour 133 is MediumOrchid3 (#af5faf)
pub const X133_MEDIUM_ORCHID3: Style = Style::new().fg(Color::Indexed(133));

/// Colour 134 is MediumOrchid (#af5fd7)
pub const X134_MEDIUM_ORCHID: Style = Style::new().fg(Color::Indexed(134));

/// Colour 135 is MediumPurple2 (#af5fff)
pub const X135_MEDIUM_PURPLE2: Style = Style::new().fg(Color::Indexed(135));

/// Colour 136 is DarkGoldenrod (#af8700)
pub const X136_DARK_GOLDENROD: Style = Style::new().fg(Color::Indexed(136));

/// Colour 137 is LightSalmon3 (#af875f)
pub const X137_LIGHT_SALMON3: Style = Style::new().fg(Color::Indexed(137));

/// Colour 138 is RosyBrown (#af8787)
pub const X138_ROSY_BROWN: Style = Style::new().fg(Color::Indexed(138));

/// Colour 139 is Grey63 (#af87af)
pub const X139_GREY63: Style = Style::new().fg(Color::Indexed(139));

/// Colour 140 is MediumPurple2 (#af87d7)
pub const X140_MEDIUM_PURPLE2: Style = Style::new().fg(Color::Indexed(140));

/// Colour 141 is MediumPurple1 (#af87ff)
pub const X141_MEDIUM_PURPLE1: Style = Style::new().fg(Color::Indexed(141));

/// Colour 142 is Gold3 (#afaf00)
pub const X142_GOLD3: Style = Style::new().fg(Color::Indexed(142));

/// Colour 143 is DarkKhaki (#afaf5f)
pub const X143_DARK_KHAKI: Style = Style::new().fg(Color::Indexed(143));

/// Colour 144 is NavajoWhite3 (#afaf87)
pub const X144_NAVAJO_WHITE3: Style = Style::new().fg(Color::Indexed(144));

/// Colour 145 is Grey69 (#afafaf)
pub const X145_GREY69: Style = Style::new().fg(Color::Indexed(145));

/// Colour 146 is LightSteelBlue3 (#afafd7)
pub const X146_LIGHT_STEEL_BLUE3: Style = Style::new().fg(Color::Indexed(146));

/// Colour 147 is LightSteelBlue (#afafff)
pub const X147_LIGHT_STEEL_BLUE: Style = Style::new().fg(Color::Indexed(147));

/// Colour 148 is Yellow3 (#afd700)
pub const X148_YELLOW3: Style = Style::new().fg(Color::Indexed(148));

/// Colour 149 is DarkOliveGreen3 (#afd75f)
pub const X149_DARK_OLIVE_GREEN3: Style = Style::new().fg(Color::Indexed(149));

/// Colour 150 is DarkSeaGreen3 (#afd787)
pub const X150_DARK_SEA_GREEN3: Style = Style::new().fg(Color::Indexed(150));

/// Colour 151 is DarkSeaGreen2 (#afd7af)
pub const X151_DARK_SEA_GREEN2: Style = Style::new().fg(Color::Indexed(151));

/// Colour 152 is LightCyan3 (#afd7d7)
pub const X152_LIGHT_CYAN3: Style = Style::new().fg(Color::Indexed(152));

/// Colour 153 is LightSkyBlue1 (#afd7ff)
pub const X153_LIGHT_SKY_BLUE1: Style = Style::new().fg(Color::Indexed(153));

/// Colour 154 is GreenYellow (#afff00)
pub const X154_GREEN_YELLOW: Style = Style::new().fg(Color::Indexed(154));

/// Colour 155 is DarkOliveGreen2 (#afff5f)
pub const X155_DARK_OLIVE_GREEN2: Style = Style::new().fg(Color::Indexed(155));

/// Colour 156 is PaleGreen1 (#afff87)
pub const X156_PALE_GREEN1: Style = Style::new().fg(Color::Indexed(156));

/// Colour 157 is DarkSeaGreen2 (#afffaf)
pub const X157_DARK_SEA_GREEN2: Style = Style::new().fg(Color::Indexed(157));

/// Colour 158 is DarkSeaGreen1 (#afffd7)
pub const X158_DARK_SEA_GREEN1: Style = Style::new().fg(Color::Indexed(158));

/// Colour 159 is PaleTurquoise1 (#afffff)
pub const X159_PALE_TURQUOISE1: Style = Style::new().fg(Color::Indexed(159));

/// Colour 160 is Red3 (#d70000)
pub const X160_RED3: Style = Style::new().fg(Color::Indexed(160));

/// Colour 161 is DeepPink3 (#d7005f)
pub const X161_DEEP_PINK3: Style = Style::new().fg(Color::Indexed(161));

/// Colour 162 is DeepPink3 (#d70087)
pub const X162_DEEP_PINK3: Style = Style::new().fg(Color::Indexed(162));

/// Colour 163 is Magenta3 (#d700af)
pub const X163_MAGENTA3: Style = Style::new().fg(Color::Indexed(163));

/// Colour 164 is Magenta3 (#d700d7)
pub const X164_MAGENTA3: Style = Style::new().fg(Color::Indexed(164));

/// Colour 165 is Magenta2 (#d700ff)
pub const X165_MAGENTA2: Style = Style::new().fg(Color::Indexed(165));

/// Colour 166 is DarkOrange3 (#d75f00)
pub const X166_DARK_ORANGE3: Style = Style::new().fg(Color::Indexed(166));

/// Colour 167 is IndianRed (#d75f5f)
pub const X167_INDIAN_RED: Style = Style::new().fg(Color::Indexed(167));

/// Colour 168 is HotPink3 (#d75f87)
pub const X168_HOT_PINK3: Style = Style::new().fg(Color::Indexed(168));

/// Colour 169 is HotPink2 (#d75faf)
pub const X169_HOT_PINK2: Style = Style::new().fg(Color::Indexed(169));

/// Colour 170 is Orchid (#d75fd7)
pub const X170_ORCHID: Style = Style::new().fg(Color::Indexed(170));

/// Colour 171 is MediumOrchid1 (#d75fff)
pub const X171_MEDIUM_ORCHID1: Style = Style::new().fg(Color::Indexed(171));

/// Colour 172 is Orange3 (#d78700)
pub const X172_ORANGE3: Style = Style::new().fg(Color::Indexed(172));

/// Colour 173 is LightSalmon3 (#d7875f)
pub const X173_LIGHT_SALMON3: Style = Style::new().fg(Color::Indexed(173));

/// Colour 174 is LightPink3 (#d78787)
pub const X174_LIGHT_PINK3: Style = Style::new().fg(Color::Indexed(174));

/// Colour 175 is Pink3 (#d787af)
pub const X175_PINK3: Style = Style::new().fg(Color::Indexed(175));

/// Colour 176 is Plum3 (#d787d7)
pub const X176_PLUM3: Style = Style::new().fg(Color::Indexed(176));

/// Colour 177 is Violet (#d787ff)
pub const X177_VIOLET: Style = Style::new().fg(Color::Indexed(177));

/// Colour 178 is Gold3 (#d7af00)
pub const X178_GOLD3: Style = Style::new().fg(Color::Indexed(178));

/// Colour 179 is LightGoldenrod3 (#d7af5f)
pub const X179_LIGHT_GOLDENROD3: Style = Style::new().fg(Color::Indexed(179));

/// Colour 180 is Tan (#d7af87)
pub const X180_TAN: Style = Style::new().fg(Color::Indexed(180));

/// Colour 181 is MistyRose3 (#d7afaf)
pub const X181_MISTY_ROSE3: Style = Style::new().fg(Color::Indexed(181));

/// Colour 182 is Thistle3 (#d7afd7)
pub const X182_THISTLE3: Style = Style::new().fg(Color::Indexed(182));

/// Colour 183 is Plum2 (#d7afff)
pub const X183_PLUM2: Style = Style::new().fg(Color::Indexed(183));

/// Colour 184 is Yellow3 (#d7d700)
pub const X184_YELLOW3: Style = Style::new().fg(Color::Indexed(184));

/// Colour 185 is Khaki3 (#d7d75f)
pub const X185_KHAKI3: Style = Style::new().fg(Color::Indexed(185));

/// Colour 186 is LightGoldenrod2 (#d7d787)
pub const X186_LIGHT_GOLDENROD2: Style = Style::new().fg(Color::Indexed(186));

/// Colour 187 is LightYellow3 (#d7d7af)
pub const X187_LIGHT_YELLOW3: Style = Style::new().fg(Color::Indexed(187));

/// Colour 188 is Grey84 (#d7d7d7)
pub const X188_GREY84: Style = Style::new().fg(Color::Indexed(188));

/// Colour 189 is LightSteelBlue1 (#d7d7ff)
pub const X189_LIGHT_STEEL_BLUE1: Style = Style::new().fg(Color::Indexed(189));

/// Colour 190 is Yellow2 (#d7ff00)
pub const X190_YELLOW2: Style = Style::new().fg(Color::Indexed(190));

/// Colour 191 is DarkOliveGreen1 (#d7ff5f)
pub const X191_DARK_OLIVE_GREEN1: Style = Style::new().fg(Color::Indexed(191));

/// Colour 192 is DarkOliveGreen1 (#d7ff87)
pub const X192_DARK_OLIVE_GREEN1: Style = Style::new().fg(Color::Indexed(192));

/// Colour 193 is DarkSeaGreen1 (#d7ffaf)
pub const X193_DARK_SEA_GREEN1: Style = Style::new().fg(Color::Indexed(193));

/// Colour 194 is Honeydew2 (#d7ffd7)
pub const X194_HONEYDEW2: Style = Style::new().fg(Color::Indexed(194));

/// Colour 195 is LightCyan1 (#d7ffff)
pub const X195_LIGHT_CYAN1: Style = Style::new().fg(Color::Indexed(195));

/// Colour 196 is Red1 (#ff0000)
pub const X196_RED1: Style = Style::new().fg(Color::Indexed(196));

/// Colour 197 is DeepPink2 (#ff005f)
pub const X197_DEEP_PINK2: Style = Style::new().fg(Color::Indexed(197));

/// Colour 198 is DeepPink1 (#ff0087)
pub const X198_DEEP_PINK1: Style = Style::new().fg(Color::Indexed(198));

/// Colour 199 is DeepPink1 (#ff00af)
pub const X199_DEEP_PINK1: Style = Style::new().fg(Color::Indexed(199));

/// Colour 200 is Magenta2 (#ff00d7)
pub const X200_MAGENTA2: Style = Style::new().fg(Color::Indexed(200));

/// Colour 201 is Magenta1 (#ff00ff)
pub const X201_MAGENTA1: Style = Style::new().fg(Color::Indexed(201));

/// Colour 202 is OrangeRed1 (#ff5f00)
pub const X202_ORANGE_RED1: Style = Style::new().fg(Color::Indexed(202));

/// Colour 203 is IndianRed1 (#ff5f5f)
pub const X203_INDIAN_RED1: Style = Style::new().fg(Color::Indexed(203));

/// Colour 204 is IndianRed1 (#ff5f87)
pub const X204_INDIAN_RED1: Style = Style::new().fg(Color::Indexed(204));

/// Colour 205 is HotPink (#ff5faf)
pub const X205_HOT_PINK: Style = Style::new().fg(Color::Indexed(205));

/// Colour 206 is HotPink (#ff5fd7)
pub const X206_HOT_PINK: Style = Style::new().fg(Color::Indexed(206));

/// Colour 207 is MediumOrchid1 (#ff5fff)
pub const X207_MEDIUM_ORCHID1: Style = Style::new().fg(Color::Indexed(207));

/// Colour 208 is DarkOrange (#ff8700)
pub const X208_DARK_ORANGE: Style = Style::new().fg(Color::Indexed(208));

/// Colour 209 is Salmon1 (#ff875f)
pub const X209_SALMON1: Style = Style::new().fg(Color::Indexed(209));

/// Colour 210 is LightCoral (#ff8787)
pub const X210_LIGHT_CORAL: Style = Style::new().fg(Color::Indexed(210));

/// Colour 211 is PaleVioletRed1 (#ff87af)
pub const X211_PALE_VIOLET_RED1: Style = Style::new().fg(Color::Indexed(211));

/// Colour 212 is Orchid2 (#ff87d7)
pub const X212_ORCHID2: Style = Style::new().fg(Color::Indexed(212));

/// Colour 213 is Orchid1 (#ff87ff)
pub const X213_ORCHID1: Style = Style::new().fg(Color::Indexed(213));

/// Colour 214 is Orange1 (#ffaf00)
pub const X214_ORANGE1: Style = Style::new().fg(Color::Indexed(214));

/// Colour 215 is SandyBrown (#ffaf5f)
pub const X215_SANDY_BROWN: Style = Style::new().fg(Color::Indexed(215));

/// Colour 216 is LightSalmon1 (#ffaf87)
pub const X216_LIGHT_SALMON1: Style = Style::new().fg(Color::Indexed(216));

/// Colour 217 is LightPink1 (#ffafaf)
pub const X217_LIGHT_PINK1: Style = Style::new().fg(Color::Indexed(217));

/// Colour 218 is Pink1 (#ffafd7)
pub const X218_PINK1: Style = Style::new().fg(Color::Indexed(218));

/// Colour 219 is Plum1 (#ffafff)
pub const X219_PLUM1: Style = Style::new().fg(Color::Indexed(219));

/// Colour 220 is Gold1 (#ffd700)
pub const X220_GOLD1: Style = Style::new().fg(Color::Indexed(220));

/// Colour 221 is LightGoldenrod2 (#ffd75f)
pub const X221_LIGHT_GOLDENROD2: Style = Style::new().fg(Color::Indexed(221));

/// Colour 222 is LightGoldenrod2 (#ffd787)
pub const X222_LIGHT_GOLDENROD2: Style = Style::new().fg(Color::Indexed(222));

/// Colour 223 is NavajoWhite1 (#ffd7af)
pub const X223_NAVAJO_WHITE1: Style = Style::new().fg(Color::Indexed(223));

/// Colour 224 is MistyRose1 (#ffd7d7)
pub const X224_MISTY_ROSE1: Style = Style::new().fg(Color::Indexed(224));

/// Colour 225 is Thistle1 (#ffd7ff)
pub const X225_THISTLE1: Style = Style::new().fg(Color::Indexed(225));

/// Colour 226 is Yellow1 (#ffff00)
pub const X226_YELLOW1: Style = Style::new().fg(Color::Indexed(226));

/// Colour 227 is LightGoldenrod1 (#ffff5f)
pub const X227_LIGHT_GOLDENROD1: Style = Style::new().fg(Color::Indexed(227));

/// Colour 228 is Khaki1 (#ffff87)
pub const X228_KHAKI1: Style = Style::new().fg(Color::Indexed(228));

/// Colour 229 is Wheat1 (#ffffaf)
pub const X229_WHEAT1: Style = Style::new().fg(Color::Indexed(229));

/// Colour 230 is Cornsilk1 (#ffffd7)
pub const X230_CORNSILK1: Style = Style::new().fg(Color::Indexed(230));

/// Colour 231 is Grey100 (#ffffff)
pub const X231_GREY100: Style = Style::new().fg(Color::Indexed(231));

/// Colour 232 is Grey3 (#080808)
pub const X232_GREY3: Style = Style::new().fg(Color::Indexed(232));

/// Colour 233 is Grey7 (#121212)
pub const X233_GREY7: Style = Style::new().fg(Color::Indexed(233));

/// Colour 234 is Grey11 (#1c1c1c)
pub const X234_GREY11: Style = Style::new().fg(Color::Indexed(234));

/// Colour 235 is Grey15 (#262626)
pub const X235_GREY15: Style = Style::new().fg(Color::Indexed(235));

/// Colour 236 is Grey19 (#303030)
pub const X236_GREY19: Style = Style::new().fg(Color::Indexed(236));

/// Colour 237 is Grey23 (#3a3a3a)
pub const X237_GREY23: Style = Style::new().fg(Color::Indexed(237));

/// Colour 238 is Grey27 (#444444)
pub const X238_GREY27: Style = Style::new().fg(Color::Indexed(238));

/// Colour 239 is Grey30 (#4e4e4e)
pub const X239_GREY30: Style = Style::new().fg(Color::Indexed(239));

/// Colour 240 is Grey35 (#585858)
pub const X240_GREY35: Style = Style::new().fg(Color::Indexed(240));

/// Colour 241 is Grey39 (#626262)
pub const X241_GREY39: Style = Style::new().fg(Color::Indexed(241));

/// Colour 242 is Grey42 (#6c6c6c)
pub const X242_GREY42: Style = Style::new().fg(Color::Indexed(242));

/// Colour 243 is Grey46 (#767676)
pub const X243_GREY46: Style = Style::new().fg(Color::Indexed(243));

/// Colour 244 is Grey50 (#808080)
pub const X244_GREY50: Style = Style::new().fg(Color::Indexed(244));

/// Colour 245 is Grey54 (#8a8a8a)
pub const X245_GREY54: Style = Style::new().fg(Color::Indexed(245));

/// Colour 246 is Grey58 (#949494)
pub const X246_GREY58: Style = Style::new().fg(Color::Indexed(246));

/// Colour 247 is Grey62 (#9e9e9e)
pub const X247_GREY62: Style = Style::new().fg(Color::Indexed(247));

/// Colour 248 is Grey66 (#a8a8a8)
pub const X248_GREY66: Style = Style::new().fg(Color::Indexed(248));

/// Colour 249 is Grey70 (#b2b2b2)
pub const X249_GREY70: Style = Style::new().fg(Color::Indexed(249));

/// Colour 250 is Grey74 (#bcbcbc)
pub const X250_GREY74: Style = Style::new().fg(Color::Indexed(250));

/// Colour 251 is Grey78 (#c6c6c6)
pub const X251_GREY78: Style = Style::new().fg(Color::Indexed(251));

/// Colour 252 is Grey82 (#d0d0d0)
pub const X252_GREY82: Style = Style::new().fg(Color::Indexed(252));

/// Colour 253 is Grey85 (#dadada)
pub const X253_GREY85: Style = Style::new().fg(Color::Indexed(253));

/// Colour 254 is Grey89 (#e4e4e4)
pub const X254_GREY89: Style = Style::new().fg(Color::Indexed(254));

/// Colour 255 is Grey93 (#eeeeee)
pub const X255_GREY93: Style = Style::new().fg(Color::Indexed(255));
