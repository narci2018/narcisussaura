import type { ExpertLayer } from '../../types';
import { Navigation } from './Navigation';
import { Dashboard } from './Dashboard';
import { ServerList } from './ServerList';
import { ChainedProxyView } from './ChainedProxyView';
import { PsiphonView } from './PsiphonView';
import { VPNGateView } from './VPNGateView';
import { ResidentialView } from './ResidentialView';
import { MegaVView } from './MegaVView';
import { SubscriptionsView } from './SubscriptionsView';
import { ImportModal } from './ImportModal';
import { SettingsView } from './SettingsView';

// Android 专家模式视图层:初始为 PC 布局的逐字快照(行为与拆分前一致),
// 之后的移动端适配只改本目录,永不回流到 pc/。
export const androidExpert: ExpertLayer = {
  Navigation,
  views: {
    dashboard: Dashboard,
    servers: ServerList,
    chains: ChainedProxyView,
    psiphon: PsiphonView,
    vpngate: VPNGateView,
    residential: ResidentialView,
    megav: MegaVView,
    subscriptions: SubscriptionsView,
    import: ImportModal,
    settings: SettingsView,
  },
};
