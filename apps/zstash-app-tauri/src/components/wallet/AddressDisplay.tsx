import { QRCodeSVG } from 'qrcode.react';
import { Copy, Check } from 'lucide-react';
import { useState } from 'react';
import type * as IPC from '../../types/ipc';
import { Button } from '../ui/button';

export function AddressDisplay(props: { address: IPC.AddressInfo }) {
  const { address } = props;
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(address.encoded);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <span className="text-sm text-muted-foreground">Address</span>
        <code className="block text-sm break-all bg-muted px-3 py-2 rounded-lg font-mono address-text">
          {address.encoded}
        </code>
      </div>

      <div className="flex gap-6 items-start flex-wrap">
        <div className="p-4 bg-white rounded-lg">
          <QRCodeSVG value={address.encoded} size={200} />
        </div>
        <div className="space-y-3">
          <Button variant="outline" onClick={copy} className="w-full">
            {copied ? (
              <>
                <Check className="h-4 w-4 text-success" />
                Copied
              </>
            ) : (
              <>
                <Copy className="h-4 w-4" />
                Copy
              </>
            )}
          </Button>
          <p className="text-xs text-muted-foreground">
            Diversifier index: {address.diversifier_index}
          </p>
        </div>
      </div>
    </div>
  );
}
