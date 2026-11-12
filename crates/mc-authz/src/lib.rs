use std::ops::{BitAnd, BitOr, Not};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Permissions(pub u64);

impl Permissions {
    pub const NONE: Self = Self(0);

    pub const ADMINISTRATOR: Self = Self(1 << 0);
    pub const MANAGE_ROLES: Self = Self(1 << 1);
    pub const MANAGE_USERS: Self = Self(1 << 2);
    pub const MANAGE_STORAGES: Self = Self(1 << 3);
    pub const MANAGE_LIBRARIES: Self = Self(1 << 4);
    pub const RUN_SCANS: Self = Self(1 << 5);
    pub const VIEW_AUDIT_LOG: Self = Self(1 << 6);

    pub const LIBRARY_VIEW: Self = Self(1 << 10);
    pub const LIBRARY_DOWNLOAD: Self = Self(1 << 11);
    pub const LIBRARY_UPLOAD: Self = Self(1 << 12);
    pub const LIBRARY_EDIT_TAGS: Self = Self(1 << 13);
    pub const LIBRARY_DELETE_FILES: Self = Self(1 << 14);
    pub const TRANSCODE: Self = Self(1 << 15);

    pub const PLAYLIST_VIEW: Self = Self(1 << 20);
    pub const PLAYLIST_EDIT: Self = Self(1 << 21);
    pub const PLAYLIST_MANAGE: Self = Self(1 << 22);
    pub const PLAYLIST_CREATE: Self = Self(1 << 23);
    pub const PLAYLIST_SHARE: Self = Self(1 << 24);
    pub const PLAYLIST_VIEW_ALL: Self = Self(1 << 25);

    pub const SERVER_BITS: Self = Self(
        Self::ADMINISTRATOR.0
            | Self::MANAGE_ROLES.0
            | Self::MANAGE_USERS.0
            | Self::MANAGE_STORAGES.0
            | Self::MANAGE_LIBRARIES.0
            | Self::RUN_SCANS.0
            | Self::VIEW_AUDIT_LOG.0,
    );

    pub const LIBRARY_BITS: Self = Self(
        Self::LIBRARY_VIEW.0
            | Self::LIBRARY_DOWNLOAD.0
            | Self::LIBRARY_UPLOAD.0
            | Self::LIBRARY_EDIT_TAGS.0
            | Self::LIBRARY_DELETE_FILES.0,
    );

    pub const PLAYLIST_BITS: Self = Self(
        Self::PLAYLIST_VIEW.0
            | Self::PLAYLIST_EDIT.0
            | Self::PLAYLIST_MANAGE.0
            | Self::PLAYLIST_CREATE.0
            | Self::PLAYLIST_SHARE.0
            | Self::PLAYLIST_VIEW_ALL.0,
    );

    pub const ALL: Self = Self(
        Self::SERVER_BITS.0 | Self::LIBRARY_BITS.0 | Self::TRANSCODE.0 | Self::PLAYLIST_BITS.0,
    );

    pub const EVERYONE_DEFAULTS: Self =
        Self(Self::TRANSCODE.0 | Self::PLAYLIST_CREATE.0 | Self::PLAYLIST_SHARE.0);

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits & Self::ALL.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    #[must_use]
    pub fn union_all(values: impl IntoIterator<Item = Self>) -> Self {
        values.into_iter().fold(Self::NONE, Self::union)
    }

    #[must_use]
    pub fn is_admin(self) -> bool {
        self.contains(Self::ADMINISTRATOR)
    }

    #[must_use]
    pub fn can_view_library(self) -> bool {
        self.contains(Self::LIBRARY_VIEW) || self.is_admin()
    }

    #[must_use]
    pub fn can_view_playlist(self) -> bool {
        self.contains(Self::PLAYLIST_VIEW)
            || self.contains(Self::PLAYLIST_VIEW_ALL)
            || self.is_admin()
    }
}

impl BitOr for Permissions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl BitAnd for Permissions {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl Not for Permissions {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0 & Self::ALL.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overwrite {
    pub allow: Permissions,
    pub deny: Permissions,
}

impl Overwrite {
    #[must_use]
    pub const fn new(allow: Permissions, deny: Permissions) -> Self {
        Self { allow, deny }
    }
}

#[must_use]
pub fn resolve_server(role_permissions: impl IntoIterator<Item = Permissions>) -> Permissions {
    Permissions::union_all(role_permissions)
}

#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn resolve_resource(
    base: Permissions,
    view: Permissions,
    resource_bits: Permissions,
    everyone: Option<Overwrite>,
    role_overwrites: &[Overwrite],
    user_overwrite: Option<Overwrite>,
    is_admin: bool,
    is_owner: bool,
) -> Permissions {
    if is_admin || is_owner {
        return Permissions::ALL;
    }

    let mut permissions = base;
    if let Some(overwrite) = everyone {
        permissions = (permissions & !overwrite.deny) | overwrite.allow;
    }

    let role_deny = Permissions::union_all(role_overwrites.iter().map(|item| item.deny));
    let role_allow = Permissions::union_all(role_overwrites.iter().map(|item| item.allow));
    permissions = (permissions & !role_deny) | role_allow;

    if let Some(overwrite) = user_overwrite {
        permissions = (permissions & !overwrite.deny) | overwrite.allow;
    }

    if !permissions.contains(view) {
        permissions = permissions.difference(resource_bits);
    }

    permissions
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PermissionInfo {
    pub bit: u8,
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
}

pub const PERMISSIONS: &[PermissionInfo] = &[
    PermissionInfo {
        bit: 0,
        key: "administrator",
        label: "Администратор",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 1,
        key: "manage_roles",
        label: "Управлять ролями",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 2,
        key: "manage_users",
        label: "Управлять пользователями",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 3,
        key: "manage_storages",
        label: "Управлять хранилищами",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 4,
        key: "manage_libraries",
        label: "Управлять библиотеками",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 5,
        key: "run_scans",
        label: "Запускать сканы и задания",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 6,
        key: "view_audit_log",
        label: "Смотреть журнал",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 10,
        key: "library.view",
        label: "Видеть и слушать библиотеку",
        group: "Библиотека",
    },
    PermissionInfo {
        bit: 11,
        key: "library.download",
        label: "Скачивать оригиналы",
        group: "Библиотека",
    },
    PermissionInfo {
        bit: 12,
        key: "library.upload",
        label: "Загружать файлы",
        group: "Библиотека",
    },
    PermissionInfo {
        bit: 13,
        key: "library.edit_tags",
        label: "Редактировать теги",
        group: "Библиотека",
    },
    PermissionInfo {
        bit: 14,
        key: "library.delete_files",
        label: "Удалять файлы",
        group: "Библиотека",
    },
    PermissionInfo {
        bit: 15,
        key: "transcode",
        label: "Пользоваться транскодером",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 20,
        key: "playlist.view",
        label: "Видеть плейлист",
        group: "Плейлист",
    },
    PermissionInfo {
        bit: 21,
        key: "playlist.edit",
        label: "Редактировать плейлист",
        group: "Плейлист",
    },
    PermissionInfo {
        bit: 22,
        key: "playlist.manage",
        label: "Управлять плейлистом",
        group: "Плейлист",
    },
    PermissionInfo {
        bit: 23,
        key: "playlist.create",
        label: "Создавать плейлисты",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 24,
        key: "playlist.share",
        label: "Делиться плейлистами",
        group: "Сервер",
    },
    PermissionInfo {
        bit: 25,
        key: "playlist.view_all",
        label: "Видеть все плейлисты",
        group: "Сервер",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn perms(bits: &[Permissions]) -> Permissions {
        Permissions::union_all(bits.iter().copied())
    }

    #[test]
    fn base_permissions_are_combined() {
        let result = resolve_server([Permissions::PLAYLIST_CREATE, Permissions::TRANSCODE]);
        assert!(result.contains(Permissions::PLAYLIST_CREATE));
        assert!(result.contains(Permissions::TRANSCODE));
    }

    #[test]
    fn everyone_overwrite_is_applied() {
        let base = Permissions::LIBRARY_VIEW;
        let result = resolve_resource(
            base,
            Permissions::LIBRARY_VIEW,
            Permissions::LIBRARY_BITS,
            Some(Overwrite::new(
                Permissions::LIBRARY_DOWNLOAD,
                Permissions::NONE,
            )),
            &[],
            None,
            false,
            false,
        );
        assert!(result.contains(Permissions::LIBRARY_VIEW));
        assert!(result.contains(Permissions::LIBRARY_DOWNLOAD));
    }

    #[test]
    fn role_allow_beats_other_role_deny() {
        let result = resolve_resource(
            Permissions::NONE,
            Permissions::LIBRARY_VIEW,
            Permissions::LIBRARY_BITS,
            None,
            &[
                Overwrite::new(Permissions::NONE, Permissions::LIBRARY_VIEW),
                Overwrite::new(Permissions::LIBRARY_VIEW, Permissions::NONE),
            ],
            None,
            false,
            false,
        );
        assert!(result.contains(Permissions::LIBRARY_VIEW));
    }

    #[test]
    fn user_deny_beats_role_allow() {
        let result = resolve_resource(
            Permissions::LIBRARY_VIEW,
            Permissions::LIBRARY_VIEW,
            Permissions::LIBRARY_BITS,
            None,
            &[Overwrite::new(Permissions::LIBRARY_VIEW, Permissions::NONE)],
            Some(Overwrite::new(Permissions::NONE, Permissions::LIBRARY_VIEW)),
            false,
            false,
        );
        assert!(!result.contains(Permissions::LIBRARY_VIEW));
        assert!(!result.contains(Permissions::LIBRARY_DOWNLOAD));
    }

    #[test]
    fn no_view_clears_resource_bits() {
        let result = resolve_resource(
            Permissions::LIBRARY_EDIT_TAGS,
            Permissions::LIBRARY_VIEW,
            Permissions::LIBRARY_BITS,
            None,
            &[],
            None,
            false,
            false,
        );
        assert!(!result.contains(Permissions::LIBRARY_EDIT_TAGS));
    }

    #[test]
    fn admin_and_owner_get_all() {
        let admin = resolve_resource(
            Permissions::NONE,
            Permissions::LIBRARY_VIEW,
            Permissions::LIBRARY_BITS,
            None,
            &[],
            None,
            true,
            false,
        );
        let owner = resolve_resource(
            Permissions::NONE,
            Permissions::PLAYLIST_VIEW,
            Permissions::PLAYLIST_BITS,
            None,
            &[],
            None,
            false,
            true,
        );
        assert_eq!(admin, Permissions::ALL);
        assert_eq!(owner, Permissions::ALL);
    }

    #[test]
    fn all_fits_in_javascript_number() {
        assert!(Permissions::ALL.bits() < (1_u64 << 53));
        assert_eq!(
            perms(&[Permissions::ADMINISTRATOR]),
            Permissions::ADMINISTRATOR
        );
    }
}
