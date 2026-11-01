/**
 * The only place iconsax is imported.
 *
 * Every name below was checked against the installed `iconsax-reactjs@0.0.8`
 * export list before being added. A name that does not exist in the package
 * imports as `undefined`, React renders `<undefined />`, and the app
 * white-screens — and TypeScript does not catch it, because the package's
 * types are permissive. Verify with:
 *
 *   node -e "const fs=require('fs');const d=fs.readFileSync(
 *     'node_modules/iconsax-reactjs/dist/index.d.ts','utf8');
 *     console.log(/declare const Foo:/.test(d))"
 *
 * Names that do NOT exist in 0.0.8 and have burned time already:
 * `Key2`, `Pen`, `Rotate`, `Undo`, `Dcube`, `Note2`, `Music2`.
 */
export {
  // transport
  Play,
  PlayCircle,
  Pause,
  Stop,
  Previous,
  Next,
  Forward,
  Backward,
  Repeat,
  RepeateOne,
  Shuffle,
  VolumeHigh,
  VolumeLow,
  VolumeMute,

  // music / analysis
  Music,
  MusicSquare,
  MusicPlay,
  MusicFilter,
  Sound,
  Keyboard,
  Flash,
  FlashCircle,
  Chart,
  Chart2,
  Activity,
  Timer1,
  RecordCircle,
  Cpu,

  // library
  Folder,
  FolderAdd,
  FolderOpen,
  SearchNormal,
  SearchZoomIn,
  SearchZoomOut,
  Filter,
  Sort,
  ArrowSwapVertical,
  Category,
  RowVertical,
  Star,
  Star1,
  Heart,

  // editing
  Add,
  AddCircle,
  Minus,
  Edit2,
  Copy,
  Trash,
  Tag,
  TextalignLeft,
  Magicpen,
  Scissor,

  // navigation / chrome
  ArrowDown2,
  ArrowUp2,
  ArrowLeft2,
  ArrowRight2,
  More,
  CloseCircle,
  Coffee,
  DocumentUpload,
  Refresh,
  Setting2,
  SliderHorizontal,
  Eye,
  EyeSlash,
  Sun1,
  Moon,
  HamburgerMenu,
  Maximize3,
  Maximize4,

  // status / feedback
  TickCircle,
  TickSquare,
  InfoCircle,
  Warning2,
  Danger,

  // files / export
  Import,
  Export,
  ExportCurve,
  DocumentText,
  DocumentDownload,
  Save2,
  Archive,
  Box,
} from 'iconsax-reactjs';

// Not iconsax glyphs — Keystone's own mark.
export { LogoMark } from './LogoMark';
export { Logo } from './Logo';
