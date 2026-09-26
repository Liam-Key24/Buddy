import { useEffect, useState, type ReactNode } from "react";
import { Camera, SignOut, SlidersHorizontal, UserCircle, UsersThree } from "@phosphor-icons/react";
import { fetchMe, logout, type Me } from "../../api";
import { Button } from "../../components/ui/Button";
import { SectionHead } from "../../components/ui/SectionHead";
import { Surface } from "../../components/ui/Surface";
import { Tag } from "../../components/ui/Tag";
import {
  PlaceholderToggle,
  SettingsMasonry,
  SettingsPageHead,
  SettingsProfileRow,
  SettingsSoonBtn,
} from "./settingsUi";

function ProfileCard({
  title,
  name,
  blurb,
  show,
  onShow,
  toggleLabel,
  icon,
}: {
  title: string;
  name: string;
  blurb: string;
  show: boolean;
  onShow: (v: boolean) => void;
  toggleLabel: string;
  icon: ReactNode;
}) {
  return (
    <Surface>
      <SectionHead icon={icon} title={title} />
      <SettingsProfileRow
        avatar={
          show ? <Camera size={22} weight="duotone" /> : <UserCircle size={28} weight="duotone" />
        }
        title={name}
        subtitle={blurb}
      />
      <div className="settings-stack settings-divider">
        <PlaceholderToggle
          checked={show}
          onChange={onShow}
          label={toggleLabel}
          hint="Preview only — not saved yet"
        />
        <SettingsSoonBtn>Choose photo… (soon)</SettingsSoonBtn>
      </div>
    </Surface>
  );
}

export function GeneralSettingsPage() {
  const [me, setMe] = useState<Me | null>(null);
  const [showAvatar, setShowAvatar] = useState(true);
  const [showPartnerAvatar, setShowPartnerAvatar] = useState(true);
  const [compactSidebar, setCompactSidebar] = useState(false);
  const [confirmDeletes, setConfirmDeletes] = useState(true);
  const [signingOut, setSigningOut] = useState(false);

  useEffect(() => {
    let cancelled = false;
    fetchMe()
      .then((row) => {
        if (!cancelled) setMe(row);
      })
      .catch(() => {
        if (!cancelled) setMe(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleLogout() {
    setSigningOut(true);
    try {
      await logout();
      window.location.reload();
    } finally {
      setSigningOut(false);
    }
  }

  return (
    <>
      <SettingsPageHead
        title="Settings"
        subtitle="Profile, preferences, and this device’s signed-in account."
        action={<Tag tone="warn">Placeholders</Tag>}
      />

      <SettingsMasonry>
        <Surface>
          <SectionHead icon={<SignOut size={16} weight="duotone" />} title="Account" />
          <p className="mb-3 text-sm text-ink-soft">
            Signed in as <strong>{me?.username ?? "…"}</strong>
          </p>
          <Button tone="raised" onClick={handleLogout} disabled={signingOut || !me}>
            {signingOut ? "Signing out…" : "Log out"}
          </Button>
        </Surface>
        <ProfileCard
          title="Your profile"
          name="You"
          blurb="Photo upload arrives with accounts"
          show={showAvatar}
          onShow={setShowAvatar}
          toggleLabel="Show profile picture"
          icon={<UserCircle size={16} weight="duotone" />}
        />
        <ProfileCard
          title="Partner profile"
          name="Partner"
          blurb="For shared households later"
          show={showPartnerAvatar}
          onShow={setShowPartnerAvatar}
          toggleLabel="Show partner picture"
          icon={<UsersThree size={16} weight="duotone" />}
        />
        <Surface>
          <SectionHead
            icon={<SlidersHorizontal size={16} weight="duotone" />}
            title="Preferences"
          />
          <div className="settings-stack">
            <PlaceholderToggle
              checked={compactSidebar}
              onChange={setCompactSidebar}
              label="Prefer compact sidebar"
              hint="Local preview — wire-up later"
            />
            <PlaceholderToggle
              checked={confirmDeletes}
              onChange={setConfirmDeletes}
              label="Confirm before deleting"
              hint="Goals and events"
            />
          </div>
        </Surface>
      </SettingsMasonry>
    </>
  );
}
