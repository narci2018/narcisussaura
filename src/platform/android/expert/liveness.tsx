import React from 'react';
import { CircleCheck, CircleDashed, CircleX } from 'lucide-react';
import { LivenessProgress, NodeStatus, UnifiedNode } from '../../../types';

/**
 * 真连接测活的三态徽标。只有后台真实建联过的节点才有资格显示"可用/不可用",
 * 没测过的一律"未测"——来源列表自称在线不算结论。
 */
export const LivenessBadge: React.FC<{ status: NodeStatus }> = ({ status }) => {
  const cfg =
    status === 'alive'
      ? { Icon: CircleCheck, cls: 'bg-emerald-500/10 text-emerald-300 border-emerald-500/30', label: '可用' }
      : status === 'dead'
        ? { Icon: CircleX, cls: 'bg-red-500/10 text-red-300 border-red-500/30', label: '不可用' }
        : { Icon: CircleDashed, cls: 'bg-[#171b28] text-gray-500 border-[#23293d]', label: '未测' };
  const { Icon, cls, label } = cfg;
  return (
    <span className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-[12px] font-medium border ${cls}`}>
      <Icon className="w-3 h-3" />
      <span>{label}</span>
    </span>
  );
};

/** 可用的排前面,没测的居中,不可用的沉底。 */
export const byLiveness = (a: UnifiedNode, b: UnifiedNode) => rank(a.status) - rank(b.status);

const rank = (status: NodeStatus) => (status === 'alive' ? 0 : status === 'dead' ? 2 : 1);

/** 一轮测活多久没动静就算它已经没了。在跑时每拨一个节点(~6 秒)就有一拍,
 * 最长的合法空档是启动一个核心(10 秒)+ 写回一批结论;45 秒收不到拍就说明
 * 这一轮再也不会播报了 —— 此时必须让用户能再点一次,而不是把按钮灰死。 */
const LIVENESS_SILENCE_MS = 45_000;

export const isLivenessRunning = (progress?: LivenessProgress, now: number = Date.now()): boolean =>
  !!progress?.running && now - (progress.at ?? 0) < LIVENESS_SILENCE_MS;

/**
 * 测活进度标签。一轮跑完、跑不完、或者根本没能开始,都要留下一句话:
 * "未测"加上沉默和按钮失灵没有区别。
 */
export const LivenessProgressTag: React.FC<{ progress?: LivenessProgress }> = ({ progress }) => {
  if (!progress) return null;
  // 后台的一句话结论优先于计数。一个节点都没拨的时候,"已测 0/137"只是把
  // 静默换个写法;用户要的是为什么。
  if (progress.message) {
    const ok = !progress.aborted;
    const Icon = ok ? CircleCheck : CircleX;
    return (
      <span
        className={`flex items-start gap-1.5 text-[12px] font-medium text-right max-w-[280px] ${
          ok ? 'text-emerald-300' : 'text-red-300'
        }`}
      >
        <Icon className="w-3.5 h-3.5 shrink-0 mt-0.5" />
        <span>{progress.message}</span>
      </span>
    );
  }
  // 声称在跑却早已没声音:说清楚,并且不再挡住按钮。
  if (progress.running && !isLivenessRunning(progress)) {
    return (
      <span className="flex items-start gap-1.5 text-[12px] font-medium text-right max-w-[280px] text-amber-300">
        <CircleX className="w-3.5 h-3.5 shrink-0 mt-0.5" />
        <span>{`这一轮测活已经没有响应，再点一次“测活全部节点”`}</span>
      </span>
    );
  }
  if (progress.total === 0) return null;
  const counts = `已测 ${progress.tested}/${progress.total} · 可用 ${progress.alive}`;
  if (progress.done && !progress.aborted) {
    return (
      <span className="flex items-center gap-1.5 text-[12px] font-medium text-emerald-300">
        <CircleCheck className="w-3.5 h-3.5" />
        <span>{`全部完成测活 · ${counts}`}</span>
      </span>
    );
  }
  if (progress.aborted) {
    return (
      <span className="flex items-center gap-1.5 text-[12px] font-medium text-amber-300">
        <CircleDashed className="w-3.5 h-3.5" />
        <span>{`测活已让路给当前连接 · ${counts}`}</span>
      </span>
    );
  }
  return (
    <span className="flex items-center gap-1.5 text-[12px] font-medium text-gray-400">
      <CircleDashed className="w-3.5 h-3.5 animate-pulse text-blue-400" />
      <span>{`后台真连接测活中 · ${counts}`}</span>
    </span>
  );
};
