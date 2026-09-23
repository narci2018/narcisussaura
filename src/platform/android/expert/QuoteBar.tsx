import React, { useState } from 'react';
import { Sparkles, Copy, Check, Quote } from 'lucide-react';

export const QuoteBar: React.FC = () => {
  const [copied, setCopied] = useState(false);

  const fullQuoteText = `致敬推动文明进步的每一个人。
他们争取到的光，会穿越漫长岁月，照亮世间每一个人，包括你。
——无名氏

A tribute to everyone who has helped advance human civilization.
The light they fought for will travel across the ages, illuminating the lives of everyone in this world—including you.
— Anonymous`;

  const handleCopy = () => {
    navigator.clipboard.writeText(fullQuoteText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="w-full max-w-3xl mx-auto my-3.5 px-5 py-3 rounded-2xl bg-gradient-to-r from-[#0b0f1e]/95 via-[#121930]/95 to-[#0b0f1e]/95 border border-[#212b48] hover:border-cyan-500/50 shadow-xl shadow-black/40 backdrop-blur-md flex items-center justify-between gap-4 relative overflow-hidden group select-text transition-all duration-300">
      {/* Background Cyber Glow & Accent Lines */}
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_75%_55%_at_50%_0%,rgba(56,189,248,0.1),transparent_70%)] pointer-events-none" />
      <div className="absolute top-0 left-1/6 right-1/6 h-[1px] bg-gradient-to-r from-transparent via-cyan-400/40 to-transparent pointer-events-none" />
      <div className="absolute bottom-0 left-1/4 right-1/4 h-[1px] bg-gradient-to-r from-transparent via-indigo-500/20 to-transparent pointer-events-none" />

      {/* Left: Cyber Badge */}
      <div className="flex items-center gap-2.5 shrink-0 select-none pointer-events-none">
        <div className="relative flex items-center justify-center">
          <div className="w-8 h-8 rounded-xl bg-gradient-to-br from-amber-500/20 via-cyan-500/20 to-indigo-600/30 border border-amber-400/40 flex items-center justify-center text-amber-300 shadow-[0_0_12px_rgba(245,158,11,0.25)]">
            <Sparkles className="w-4 h-4 animate-pulse text-amber-300" />
          </div>
          <div className="absolute inset-0 rounded-xl border border-cyan-400/20 animate-ping opacity-25 pointer-events-none" />
        </div>
        <div className="hidden sm:flex flex-col">
          <span className="text-[10px] font-mono font-bold tracking-wider uppercase text-transparent bg-clip-text bg-gradient-to-r from-amber-300 via-cyan-300 to-indigo-200">
            文明之光
          </span>
          <span className="text-[8px] font-mono tracking-widest text-gray-500 -mt-0.5">
            CIVILIZATION
          </span>
        </div>
      </div>

      {/* Center: Dual-Line Quotes (Fully Visible, No Truncation) */}
      <div className="flex-1 flex flex-col justify-center min-w-0 px-2 text-left">
        {/* Chinese Quote Line */}
        <div className="flex flex-wrap items-center gap-1.5 text-xs sm:text-[12.5px] leading-relaxed font-medium text-gray-200">
          <Quote className="w-3 h-3 text-cyan-400/80 shrink-0 rotate-180 -mt-0.5" />
          <span>
            致敬推动<span className="text-cyan-300 font-semibold mx-0.5">文明进步</span>的每一个人。他们争取到的
            <span className="text-amber-300 font-bold mx-0.5 drop-shadow-[0_0_6px_rgba(252,211,77,0.4)]">光</span>
            ，会穿越漫长岁月，照亮世间每一个人，包括你。
          </span>
          <span className="text-[11px] text-gray-400/80 font-mono shrink-0 ml-1">
            —— 无名氏
          </span>
        </div>

        {/* English Quote Line */}
        <div className="text-[10px] sm:text-[11px] leading-relaxed text-gray-400/90 font-sans tracking-wide mt-1">
          <span className="italic">
            A tribute to everyone who has helped advance human civilization. The light they fought for will travel across the ages, illuminating the lives of everyone in this world—including you.
          </span>
          <span className="text-[9.5px] text-gray-500 font-mono shrink-0 ml-1.5 not-italic">
            — Anonymous
          </span>
        </div>
      </div>

      {/* Right: Quick Copy Button */}
      <div className="flex items-center shrink-0 select-none">
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 px-2.5 py-1.5 rounded-xl bg-[#141b2f]/90 hover:bg-[#1e2744] text-gray-400 hover:text-cyan-300 border border-[#232f50] hover:border-cyan-500/50 transition-all text-[10px] font-mono shadow-sm group/btn"
          title="复制中英文名言至剪贴板"
        >
          {copied ? (
            <>
              <Check className="w-3 h-3 text-emerald-400 animate-in zoom-in" />
              <span className="text-emerald-400 font-medium">已复制</span>
            </>
          ) : (
            <>
              <Copy className="w-3 h-3 text-gray-400 group-hover/btn:text-cyan-300 transition-colors" />
              <span className="hidden sm:inline">复制</span>
            </>
          )}
        </button>
      </div>
    </div>
  );
};
