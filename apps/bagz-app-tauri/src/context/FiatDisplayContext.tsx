import { createContext, useContext, ReactNode } from 'react';
import { useFiatDisplay } from '../hooks/useFiatDisplay';

type FiatDisplayContextValue = ReturnType<typeof useFiatDisplay>;

const FiatDisplayContext = createContext<FiatDisplayContextValue | null>(null);

export function FiatDisplayProvider({ children }: { children: ReactNode }) {
  const fiatDisplay = useFiatDisplay();
  return (
    <FiatDisplayContext.Provider value={fiatDisplay}>
      {children}
    </FiatDisplayContext.Provider>
  );
}

export function useFiatDisplayContext() {
  const context = useContext(FiatDisplayContext);
  if (!context) {
    throw new Error('useFiatDisplayContext must be used within FiatDisplayProvider');
  }
  return context;
}
