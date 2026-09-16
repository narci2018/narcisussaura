import React, { useState } from 'react';
import { Lock, ShieldAlert, X, Eye, EyeOff } from 'lucide-react';

interface ExpertModeGateProps {
  onConfirm: () => void;
  onCancel: () => void;
}

const EXPERT_PASSWORD = 'Tocean@788';

export const ExpertModeGate: React.FC<ExpertModeGateProps> = ({ onConfirm, onCancel }) => {
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState('');
  const [shake, setShake] = useState(false);

  const handleConfirm = () => {
    if (password === EXPERT_PASSWORD) {
      onConfirm();
    } else {
      setError('密码错误，请重试');
      setShake(true);
      setPassword('');
      setTimeout(() => setShake(false), 500);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleConfirm();
    if (e.key === 'Escape') onCancel();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div
        className={`relative bg-[#0f1219] border border-[#2a3050] rounded-3xl shadow-2xl shadow-black/60 p-8 mx-4 w-full max-w-sm transition-all ${shake ? 'animate-[shake_0.4s_ease-in-out]' : ''}`}
        style={{
          animation: shake ? 'shake 0.4s ease-in-out' : undefined,
        }}
      >
        {/* Close button */}
        <button
          onClick={onCancel}
          className="absolute top-4 right-4 text-gray-500 hover:text-gray-300 transition-colors"
        >
          <X className="w-5 h-5" />
        </button>

        {/* Warning header */}
        <div className="flex flex-col items-center mb-6">
          <div className="w-16 h-16 rounded-2xl bg-amber-500/10 border border-amber-500/30 flex items-center justify-center mb-4">
            <ShieldAlert className="w-8 h-8 text-amber-400" />
          </div>
          <h2 className="text-lg font-bold text-gray-100">切换专家模式</h2>
          <p className="text-center text-sm text-amber-400/90 mt-2 leading-relaxed px-2">
            ⚠️ 建议非IT行业人士保持小白模式！
          </p>
          <p className="text-center text-xs text-gray-500 mt-1 leading-relaxed px-2">
            专家模式提供完整配置权限，误操作可能影响正常使用。
          </p>
        </div>

        {/* Lock icon + input */}
        <div className="mb-2">
          <label className="text-xs text-gray-400 font-medium mb-2 flex items-center gap-1.5">
            <Lock className="w-3.5 h-3.5" />
            输入访问密码
          </label>
          <div className="relative">
            <input
              type={showPassword ? 'text' : 'password'}
              value={password}
              onChange={(e) => {
                setPassword(e.target.value);
                setError('');
              }}
              onKeyDown={handleKeyDown}
              autoFocus
              placeholder="请输入密码"
              className={`w-full bg-[#181d2a] border rounded-xl px-4 py-3 text-sm text-gray-200 placeholder-gray-600 outline-none focus:border-indigo-500/60 transition-colors pr-10 ${
                error ? 'border-red-500/50' : 'border-[#252c3e]'
              }`}
            />
            <button
              type="button"
              onClick={() => setShowPassword(!showPassword)}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300"
            >
              {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            </button>
          </div>
          {error && (
            <p className="text-xs text-red-400 mt-1.5 flex items-center gap-1">
              <span>✕</span> {error}
            </p>
          )}
        </div>

        {/* Buttons */}
        <div className="flex gap-3 mt-6">
          <button
            onClick={onCancel}
            className="flex-1 py-2.5 rounded-xl text-sm font-medium text-gray-400 bg-[#181d2a] border border-[#252c3e] hover:border-[#3a4258] hover:text-gray-200 transition-all"
          >
            取消
          </button>
          <button
            onClick={handleConfirm}
            className="flex-1 py-2.5 rounded-xl text-sm font-bold text-white bg-gradient-to-br from-indigo-600 to-violet-700 hover:from-indigo-500 hover:to-violet-600 transition-all shadow-lg shadow-indigo-900/40"
          >
            确认进入
          </button>
        </div>
      </div>

      <style>{`
        @keyframes shake {
          0%, 100% { transform: translateX(0); }
          20% { transform: translateX(-8px); }
          40% { transform: translateX(8px); }
          60% { transform: translateX(-6px); }
          80% { transform: translateX(6px); }
        }
      `}</style>
    </div>
  );
};
