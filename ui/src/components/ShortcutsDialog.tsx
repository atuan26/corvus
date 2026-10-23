import { Dialog, Heading, Modal, ModalOverlay, Button } from "react-aria-components";
import { GLOBAL_SHORTCUTS, SHORTCUTS } from "../shortcuts";

interface Props {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ShortcutsDialog({ isOpen, onOpenChange }: Props) {
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
                  {GLOBAL_SHORTCUTS.map((s) => (
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
