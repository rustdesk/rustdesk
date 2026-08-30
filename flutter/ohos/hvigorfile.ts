import { appTasks } from '@ohos/hvigor-ohos-plugin';
import { flutterHvigorPlugin } from 'flutter-hvigor-plugin';
import path from 'path';

export default {
  system: appTasks,
  plugins: [flutterHvigorPlugin(path.dirname(__dirname))]
}
