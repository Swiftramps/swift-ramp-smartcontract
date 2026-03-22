import type { Metadata } from 'next'
import './globals.css'

export const metadata: Metadata = {
  title: 'StellarFund – Crowdfunding on Stellar',
  description: 'Fund projects with XLM and USDC on the Stellar network',
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <link href="https://fonts.googleapis.com/css2?family=Syne:wght@400;600;700;800&family=DM+Sans:wght@300;400;500&display=swap" rel="stylesheet" />
      </head>
      <body style={{ fontFamily: "'DM Sans', sans-serif", background: '#0a0a0f', color: '#f0ece4' }}>{children}</body>
    </html>
  )
}
