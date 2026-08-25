export type Subscriber<T> = (value: T) => void;
export type Unsubscribe = () => void;

export type Signal<T> = {
  get(): T;
  set(value: T): void;
  subscribe(fn: Subscriber<T>): Unsubscribe;
};

export function signal<T>(initial: T): Signal<T> {
  let value = initial;
  const subs = new Set<Subscriber<T>>();
  return {
    get() {
      return value;
    },
    set(next: T) {
      value = next;
      for (const fn of [...subs]) {
        fn(value);
      }
    },
    subscribe(fn: Subscriber<T>): Unsubscribe {
      subs.add(fn);
      return () => {
        subs.delete(fn);
      };
    },
  };
}
