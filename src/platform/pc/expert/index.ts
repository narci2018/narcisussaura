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

// PC(windows+macOS)专家模式视图层。手机专家模式在 ../android/expert/,物理独立,互不影响。
export const pcExpert: ExpertLayer = {
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
