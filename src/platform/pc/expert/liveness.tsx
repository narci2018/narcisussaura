import React from 'react';
import { CircleCheck, CircleDashed, CircleX } from 'lucide-react';
import { LivenessProgress, NodeStatus, UnifiedNode } from '../../../types';

/**
 * 真连接测活的三态徽标。只有后台真实建联过的节点才有资格显示"可用/不可用",
 * 没测过的一律"未测"——来源列表自称在线不算结论(v0.2.103 之前的谎报就是这么来的)。
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
    <span className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium border ${cls}`}>
      <Icon className="w-3 h-3" />
      <span>{label}</span>
    </span>
  );
};

/** 可用的排前面,没测的居中,不可用的沉底。 */
export const byLiveness = (a: UnifiedNode, b: UnifiedNode) => rank(a.status) - rank(b.status);

const rank = (status: NodeStatus) => (status === 'alive' ? 0 : status === 'dead' ? 2 : 1);

/**
 * 测活进度标签。后台一轮都没开始时不显示;跑完整个名单时明确告诉用户
 * "全部完成测活",中途让路给真实隧道时说明还剩多少没测。
 */
export const LivenessProgressTag: React.FC<{ progress?: LivenessProgress }> = ({ progress }) => {
  if (!progress || progress.total === 0) return null;
  const counts = `已测 ${progress.tested}/${progress.total} · 可用 ${progress.alive}`;
  if (progress.done && !progress.aborted) {
    return (
      <span className="flex items-center gap-1.5 text-[11px] font-medium text-emerald-300">
        <CircleCheck className="w-3.5 h-3.5" />
        <span>{`全部完成测活 · ${counts}`}</span>
      </span>
    );
  }
  if (progress.aborted) {
    return (
      <span className="flex items-center gap-1.5 text-[11px] font-medium text-amber-300">
        <CircleDashed className="w-3.5 h-3.5" />
        <span>{`测活已让路给当前连接 · ${counts}`}</span>
      </span>
    );
  }
  return (
    <span className="flex items-center gap-1.5 text-[11px] font-medium text-gray-400">
      <CircleDashed className="w-3.5 h-3.5 animate-pulse text-blue-400" />
      <span>{`后台真连接测活中 · ${counts}`}</span>
    </span>
  );
};
