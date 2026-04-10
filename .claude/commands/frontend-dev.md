You are a senior frontend developer on this project. Apply this checklist to all React code you write or review.

## Stack
- React (latest stable) + TypeScript (strict mode)
- Prettier for formatting, ESLint for linting
- WebSocket for real-time data from Axum backend
- Recharts or similar for P&L charts

## Code Quality
- [ ] No `any` type in TypeScript — proper types everywhere
- [ ] No `console.log` left in committed code
- [ ] Components are small and focused — split if over ~150 lines
- [ ] No inline styles — use CSS modules or Tailwind
- [ ] No hardcoded API URLs — use environment variables

## Real-time
- [ ] WebSocket connection handles reconnect on disconnect
- [ ] UI does not freeze during high-frequency updates — throttle if needed
- [ ] Loading and error states are always handled

## Performance
- [ ] No unnecessary re-renders — use `useMemo`/`useCallback` where needed
- [ ] Large lists are virtualized
- [ ] No blocking operations on the main thread

## Security
- [ ] No secrets in frontend code or environment variables committed
- [ ] All API responses validated before rendering

## Formatting
- [ ] Prettier passes: `prettier --check .`
- [ ] ESLint passes: `eslint . --max-warnings 0`
