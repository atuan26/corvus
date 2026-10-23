import { Dialog, Heading, Modal, ModalOverlay, Button } from "react-aria-components";
import type { ShortcutKeys } from "../api";
import { GLOBAL_LABELS, PROVIDER_SHORTCUT, SHORTCUTS } from "../shortcuts";

interface Props {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  globalKeys: ShortcutKeys | null;
}

export function ShortcutsDialog({ isOpen, onOpenChange, globalKeys }: Props) {
  const globals = [
    PROVIDER_SHORTCUT,
    ...(Object.keys(GLOBAL_LABELS) as (keyof ShortcutKeys)[])
      .filter((k) => globalKeys?.[k])
      .map((k) => ({ keys: globalKeys![k], label: GLOBAL_LABELS[k] })),
  ];
  return (
    <ModalOverlay className="overlay" isOpen={isOpen} onOpenChange={onOpenChange} isDismissable>
      <Modal className="modal">
        <Dialog className="dialog">
          {({ close }) => (
            <>
              <Heading slot="title" className="dialog-title">
                Keyboard shortcuts
              </Heading>
              <table className="shortcut-table">
                <caption className="sr-only">In this window</caption>
                <tbody>
                  {SHORTCUTS.map((s) => (
                    <tr key={s.action}>
                      <td>{s.label}</td>
                      <td>
                        <kbd>{s.keys}</kbd>
                      </td>
                    </tr>
                  ))}
                  {globals.map((s) => (
                    <tr key={s.keys}>
                      <td>{s.label}</td>
                      <td>
                        <kbd>{s.keys}</kbd>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="dialog-actions">
                <Button className="btn" onPress={close} autoFocus>
                  Close
                </Button>
              </div>
            </>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
