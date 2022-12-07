import { useEffect, useRef } from 'react';
import init, { init_space, render, step } from 'wasm-lib';
import styles from './index.module.css';

export default function Home() {
  const paused = useRef(false);

  useEffect(() => {
    const canvas = document.getElementById('canvas') as HTMLCanvasElement;
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;

    const run = async () => {
      await init();
      init_space();

      const loop = () => {
        if (!paused.current) step();
        render();

        requestAnimationFrame(loop);
      };

      requestAnimationFrame(loop);
    };

    run();
  }, []);

  return (
    <canvas
      className={styles.canvas}
      id="canvas"
      onKeyDown={e => {
        if (e.key === ' ') {
          paused.current = !paused.current;
        }
      }}
      tabIndex={0}
    />
  );
}
