'use client'
import { useState } from 'react'

const projects = [
  { id: 1, title: 'Solar Panels for Lagos Schools', creator: 'GreenAfrica DAO', goal: 50000, raised: 34200, backers: 214, days: 12, tag: 'Energy', color: '#f59e0b' },
  { id: 2, title: 'Open Source Stellar Dev Tools', creator: 'BuildOnStellar', goal: 20000, raised: 19800, backers: 98, days: 3, tag: 'Tech', color: '#6366f1' },
  { id: 3, title: 'Women Farmers Micro-Loans', creator: 'AgriHope Fund', goal: 30000, raised: 12500, backers: 167, days: 21, tag: 'Impact', color: '#10b981' },
  { id: 4, title: 'Afrobeat NFT Album Release', creator: 'Fela Jr.', goal: 8000, raised: 6100, backers: 432, days: 7, tag: 'Music', color: '#f43f5e' },
]

export default function Home() {
  const [donated, setDonated] = useState<number | null>(null)
  const [amount, setAmount] = useState('')

  return (
    <div style={{ minHeight: '100vh', background: '#0a0a0f' }}>
      {/* Nav */}
      <nav style={{ borderBottom: '1px solid #1e1e2e', padding: '18px 40px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ fontFamily: "'Syne', sans-serif", fontWeight: 800, fontSize: 22, letterSpacing: '-0.5px' }}>
          <span style={{ color: '#7c3aed' }}>Stellar</span>Fund
        </div>
        <div style={{ display: 'flex', gap: 32, fontSize: 14, color: '#888' }}>
          <span style={{ cursor: 'pointer', color: '#f0ece4' }}>Explore</span>
          <span style={{ cursor: 'pointer' }}>How it works</span>
          <span style={{ cursor: 'pointer' }}>Start a project</span>
        </div>
        <button style={{ background: '#7c3aed', color: '#fff', border: 'none', borderRadius: 8, padding: '10px 20px', fontSize: 14, fontWeight: 500, cursor: 'pointer' }}>
          Connect Wallet
        </button>
      </nav>

      {/* Hero */}
      <div style={{ textAlign: 'center', padding: '80px 40px 60px' }}>
        <div style={{ display: 'inline-block', background: '#1a0a2e', border: '1px solid #3b1a6e', borderRadius: 20, padding: '6px 16px', fontSize: 12, color: '#a78bfa', marginBottom: 24, letterSpacing: 1 }}>
          POWERED BY STELLAR NETWORK
        </div>
        <h1 style={{ fontFamily: "'Syne', sans-serif", fontSize: 64, fontWeight: 800, lineHeight: 1.05, marginBottom: 20, letterSpacing: '-2px' }}>
          Fund what<br /><span style={{ color: '#7c3aed' }}>matters most</span>
        </h1>
        <p style={{ color: '#888', fontSize: 18, maxWidth: 480, margin: '0 auto 40px', lineHeight: 1.6 }}>
          Back bold ideas with XLM & USDC. Near-zero fees, 5-second settlement.
        </p>
        <div style={{ display: 'flex', gap: 12, justifyContent: 'center' }}>
          <button style={{ background: '#7c3aed', color: '#fff', border: 'none', borderRadius: 10, padding: '14px 28px', fontSize: 15, fontWeight: 600, cursor: 'pointer' }}>Explore Projects</button>
          <button style={{ background: 'transparent', color: '#f0ece4', border: '1px solid #2a2a3e', borderRadius: 10, padding: '14px 28px', fontSize: 15, cursor: 'pointer' }}>Start a Campaign</button>
        </div>
      </div>

      {/* Stats */}
      <div style={{ display: 'flex', justifyContent: 'center', gap: 60, padding: '0 40px 60px', borderBottom: '1px solid #1e1e2e' }}>
        {[['$2.4M', 'Total Raised'], ['1,240', 'Projects Funded'], ['18,900', 'Backers'], ['< $0.01', 'Avg Fee']].map(([val, label]) => (
          <div key={label} style={{ textAlign: 'center' }}>
            <div style={{ fontFamily: "'Syne', sans-serif", fontSize: 32, fontWeight: 700, color: '#a78bfa' }}>{val}</div>
            <div style={{ fontSize: 13, color: '#666', marginTop: 4 }}>{label}</div>
          </div>
        ))}
      </div>

      {/* Projects */}
      <div style={{ maxWidth: 1100, margin: '0 auto', padding: '60px 40px' }}>
        <h2 style={{ fontFamily: "'Syne', sans-serif", fontSize: 28, fontWeight: 700, marginBottom: 32 }}>Live Campaigns</h2>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 20 }}>
          {projects.map(p => {
            const pct = Math.round((p.raised / p.goal) * 100)
            return (
              <div key={p.id} style={{ background: '#12121c', border: '1px solid #1e1e2e', borderRadius: 16, padding: 28, cursor: 'pointer', transition: 'border-color 0.2s' }}
                onMouseEnter={e => (e.currentTarget.style.borderColor = '#3b1a6e')}
                onMouseLeave={e => (e.currentTarget.style.borderColor = '#1e1e2e')}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 16 }}>
                  <span style={{ background: p.color + '20', color: p.color, fontSize: 11, fontWeight: 600, padding: '4px 10px', borderRadius: 20, letterSpacing: 0.5 }}>{p.tag}</span>
                  <span style={{ fontSize: 12, color: '#555' }}>{p.days}d left</span>
                </div>
                <h3 style={{ fontFamily: "'Syne', sans-serif", fontSize: 18, fontWeight: 700, marginBottom: 6, lineHeight: 1.3 }}>{p.title}</h3>
                <p style={{ fontSize: 13, color: '#666', marginBottom: 20 }}>by {p.creator}</p>
                <div style={{ background: '#1e1e2e', borderRadius: 4, height: 6, marginBottom: 12, overflow: 'hidden' }}>
                  <div style={{ background: p.color, height: '100%', width: `${pct}%`, borderRadius: 4, transition: 'width 1s' }} />
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 13 }}>
                  <span style={{ color: '#f0ece4', fontWeight: 600 }}>${p.raised.toLocaleString()} <span style={{ color: '#555', fontWeight: 400 }}>raised</span></span>
                  <span style={{ color: p.color, fontWeight: 600 }}>{pct}%</span>
                </div>
                <div style={{ fontSize: 12, color: '#555', marginTop: 6 }}>{p.backers} backers · Goal: ${p.goal.toLocaleString()}</div>
                <button
                  onClick={() => setDonated(p.id)}
                  style={{ marginTop: 20, width: '100%', background: donated === p.id ? '#10b981' : '#7c3aed', color: '#fff', border: 'none', borderRadius: 8, padding: '12px', fontSize: 14, fontWeight: 600, cursor: 'pointer', transition: 'background 0.2s' }}>
                  {donated === p.id ? '✓ Backed!' : 'Back this project'}
                </button>
              </div>
            )
          })}
        </div>
      </div>

      {/* CTA */}
      <div style={{ background: '#12121c', borderTop: '1px solid #1e1e2e', borderBottom: '1px solid #1e1e2e', padding: '60px 40px', textAlign: 'center' }}>
        <h2 style={{ fontFamily: "'Syne', sans-serif", fontSize: 36, fontWeight: 800, marginBottom: 16 }}>Have an idea worth funding?</h2>
        <p style={{ color: '#888', marginBottom: 28 }}>Launch your campaign in minutes. No bank account needed.</p>
        <button style={{ background: '#7c3aed', color: '#fff', border: 'none', borderRadius: 10, padding: '14px 32px', fontSize: 15, fontWeight: 600, cursor: 'pointer' }}>Start your campaign →</button>
      </div>

      <footer style={{ textAlign: 'center', padding: '30px', fontSize: 12, color: '#444' }}>
        Built on Stellar Network · StellarFund © 2025
      </footer>
    </div>
  )
}
