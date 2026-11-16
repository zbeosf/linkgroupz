// 보관함(관리 창 오른쪽)에서 끌기 시작한 것. 왼쪽 그룹 목록이 놓을 수 있는지 판단할 때 쓴다.
// 끄는 동안에는 dataTransfer 를 읽을 수 없어 따로 기억한다.
//
// { kind: "archivedGroups", ids } | { kind: "archivedLinks", ids } | { kind: "archivedGroupLinks", groupId, ids }
export let archiveDrag = null;

export function setArchiveDrag(value) {
  archiveDrag = value;
}
