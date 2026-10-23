import { useMemo, type Ref } from "react";
import { Button, ComboBox, Input, Label, ListBox, ListBoxItem, Popover } from "react-aria-components";
import type { Language } from "../api";
import { matchesLanguage } from "./languageFilter";

interface Props {
  label: string;
  languages: Language[];
  value: string;
  onChange: (code: string) => void;
  inputRef?: Ref<HTMLInputElement>;
}

interface Item extends Language {
  id: string;
}

export function LanguagePicker({ label, languages, value, onChange, inputRef }: Props) {
  const items = useMemo<Item[]>(() => languages.map((l) => ({ ...l, id: l.code })), [languages]);
  // The filter only receives the item's text, so look the full entry up by its English name.
  const byName = useMemo(() => new Map(items.map((i) => [i.name, i])), [items]);

  return (
    <ComboBox
      className="lang-picker"
      // `defaultItems` (not `items`) keeps React Aria's built-in filtering active.
      defaultItems={items}
      defaultFilter={(textValue, input) => {
        const lang = byName.get(textValue);
        return lang ? matchesLanguage(lang, input) : true;
      }}
      selectedKey={value}
      onSelectionChange={(key) => key != null && onChange(String(key))}
      menuTrigger="focus"
      allowsEmptyCollection
    >
      <Label className="sr-only">{label}</Label>
      <div className="field">
        <Input ref={inputRef} className="field-input" />
        <Button className="field-button" aria-label={`Show ${label.toLowerCase()} list`}>
          <span aria-hidden>▾</span>
        </Button>
      </div>
      <Popover className="popover">
        <ListBox<Item> className="listbox" renderEmptyState={() => <div className="listbox-empty">No match</div>}>
          {(item) => (
            <ListBoxItem className="listbox-item" textValue={item.name}>
              <span>{item.name}</span>
              {item.native && item.native !== item.name && (
                <span className="listbox-native" lang={item.code}>
                  {item.native}
                </span>
              )}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </ComboBox>
  );
}
