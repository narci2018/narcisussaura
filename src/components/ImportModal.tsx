import React, { useState } from 'react';
import { PlusCircle, Link, AlertCircle, ArrowLeft, Copy, Check } from 'lucide-react';
import { useAppStore } from '../stores/appStore';

export const ImportModal: React.FC = () => {
  const { importLink, setActiveTab, errorMessage, setErrorMessage } = useAppStore();
  const [shareLink, setShareLink] = useState('');
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleImport = async () => {
    if (!shareLink.trim()) return;
    setLoading(true);
    const success = await importLink(shareLink.trim());
    setLoading(false);
    if (success) {
      setShareLink('');
    }
  };

  return (
    <div className="flex-1 flex flex-col items-center justify-center px-8 py-6 max-w-2xl mx-auto w-full">
      <div className="w-full bg-[#12151f] border border-[#212637] rounded-2xl p-6 shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-xl bg-blue-500/10 border border-blue-500/20 flex items-center justify-center text-blue-400">
              <Link className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-gray-200">Import Proxy Server</h2>
              <p className="text-xs text-gray-500">Paste your proxy share link below</p>
            </div>
          </div>

          <button
            onClick={() => setActiveTab('servers')}
            className="flex items-center gap-1 text-xs text-gray-400 hover:text-gray-200"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>Back to Servers</span>
          </button>
        </div>

        {/* Input Area */}
        <div className="space-y-3">
          <label className="text-xs font-medium text-gray-300">Server Share URL</label>
          <textarea
            rows={4}
            placeholder="vless://... or trojan://... or socks5://..."
            value={shareLink}
            onChange={(e) => {
              setShareLink(e.target.value);
              setErrorMessage(null);
            }}
            className="w-full bg-[#0b0d14] border border-[#212638] rounded-xl p-3 text-xs text-gray-200 font-mono placeholder-gray-600 focus:outline-none focus:border-blue-500/60 transition-colors resize-none"
          />

          <div className="text-[11px] text-gray-500 space-y-1">
            <p>Supported standard protocol formats:</p>
            <ul className="list-disc list-inside text-gray-400">
              <li><code className="text-blue-400">vless://uuid@host:port?security=reality&pbk=...#Name</code></li>
              <li><code className="text-blue-400">trojan://password@host:port?sni=...#Name</code></li>
              <li><code className="text-blue-400">socks5://user:pass@host:port#Name</code></li>
            </ul>
          </div>
        </div>

        {/* Error Feedback */}
        {errorMessage && (
          <div className="mt-4 p-3 rounded-xl bg-[#1c1216] border border-red-500/30 text-red-300 text-xs flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 flex-1 min-w-0 select-text">
              <AlertCircle className="w-4 h-4 text-red-400 shrink-0" />
              <span className="font-mono text-[11px] break-all select-text cursor-text">{errorMessage}</span>
            </div>
            <button
              onClick={() => {
                navigator.clipboard.writeText(errorMessage);
                setCopied(true);
                setTimeout(() => setCopied(false), 2000);
              }}
              className="px-2.5 py-1 rounded-lg bg-red-500/20 hover:bg-red-500/30 text-red-200 text-[11px] font-medium transition-colors shrink-0 flex items-center gap-1 border border-red-500/30"
              title="复制错误信息"
            >
              {copied ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
              <span>{copied ? '已复制' : '复制错误'}</span>
            </button>
          </div>
        )}

        {/* Action Button */}
        <div className="mt-6 flex justify-end gap-3">
          <button
            onClick={() => setActiveTab('servers')}
            className="px-4 py-2 rounded-xl text-xs font-medium text-gray-400 hover:text-gray-200 hover:bg-[#1a1e2b] transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleImport}
            disabled={loading || !shareLink.trim()}
            className="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 disabled:opacity-40 text-white text-xs font-semibold shadow-sm shadow-blue-500/25 transition-all flex items-center gap-1.5"
          >
            <PlusCircle className="w-4 h-4" />
            <span>{loading ? 'Importing...' : 'Import Server'}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
